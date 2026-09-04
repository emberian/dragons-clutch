#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Isolated authenticated SBF adapter for the sparse canonical Market Core.
//!
//! The generated Market Core interpreter remains the semantic owner. This
//! crate owns only the Solana trust boundary: exact account frames, finalized
//! record/PDA joins, Registry/Loader-backed role reauthentication, prepaid
//! account creation, child CPI provenance, and commit-last persistence.

extern crate alloc;

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_RECEIPT_BYTES_V5, CLAIMS_FOUNDING_RECEIPT_MAGIC_V5,
};
use dclutch_claims_svm::market_closure_v1::CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
    PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1,
};
use dclutch_market_core_codec::{
    AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1, AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1,
    AGGREGATE_RETIREMENT_FINISH_MAGIC_V1, AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1, Action,
    CAPABILITY_FUNDING_HEADER_BYTES_V2, CORE_EFFECT_ENVELOPE_BYTES_V1, CORE_REQUEST_MAGIC,
    CapabilityFundingHeaderV2, CoreEffectEnvelopeV1, GENERIC_FOUNDING_REQUEST_BYTES_V1,
    GENERIC_FOUNDING_REQUEST_MAGIC_V1, GenericFoundingRequestV1, PROJECT_FOUND_REQUEST_BYTES_V2,
    PROJECT_FOUND_REQUEST_MAGIC_V2, ProjectFoundRequestV2, REQUEST_BYTES,
    RETIREMENT_BUNDLE_BYTES_V1, Request, SERIES_CORE_REQUEST_BYTES_V1,
    SERIES_CORE_REQUEST_MAGIC_V1, SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1,
    SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1, SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1,
    SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1, SeriesCoreRequestV1,
    SeriesPermitExpiryRequestV1, SeriesUnallocatedPermitExpiryRequestV1,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1, INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V2,
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V2, InitializeProtocolInfrastructureV1,
    InitializeProtocolInfrastructureV2,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

mod begin_retiring;
mod capability;
mod execute_provider_v3;
mod fixed_role;
mod found;
mod frame;
mod generic_founding_v1;
mod infrastructure;
mod infrastructure_v2;
mod open_market;
mod product_runtime_v2;
mod records;
mod release;
mod resolution;
pub mod retire_v1;
mod retirement_replay_handoff_v1;
mod series_consume;
mod series_open;
mod series_permit_expiry;
mod series_permit_expiry_precommit_v1;

pub use begin_retiring::BEGIN_RETIRING_ACCOUNT_COUNT_V1;
pub use execute_provider_v3::{
    EXECUTE_PROVIDER_ACCOUNT_COUNT_V3, EXECUTE_PROVIDER_PREFIX_BYTES_V3,
};
pub use frame::{
    FOUND_ACCOUNT_COUNT_V3, INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V1,
    INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2, PROJECTED_FOUND_ACCOUNT_COUNT_V2,
};
pub use generic_founding_v1::{
    GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1, GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
};
pub use retire_v1::{RETIREMENT_ACCOUNT_COUNT_V1, RETIREMENT_INSTRUCTION_BYTES_V1};
pub use series_consume::{
    SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1, SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V2,
};
pub use series_open::SERIES_OPEN_ACCOUNT_COUNT_V1;
pub use series_permit_expiry::SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1;
pub use series_permit_expiry_precommit_v1::SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1;

/// Exact prefix for a generic capability action before child-owned bytes.
pub const CAPABILITY_PREFIX_BYTES_V1: usize = REQUEST_BYTES + CORE_EFFECT_ENVELOPE_BYTES_V1;
/// Exact generic capability semantic prefix for subset-ledger V2 routes.
pub const CAPABILITY_ROLE_PREFIX_BYTES_V2: usize =
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1 + CAPABILITY_FUNDING_HEADER_BYTES_V2;

/// Stable refusal from the isolated Core SBF trust boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreSbfError {
    /// Instruction bytes or action-specific inactive fields refused.
    Instruction = 0x3000,
    /// Account count, order, privilege, executable flag, or alias refused.
    AccountFrame = 0x3001,
    /// Finalized record owner, PDA, cursor absence, Rent, digest, or schema refused.
    FinalizedRecord = 0x3002,
    /// Realm/Product/result-domain/Market identity linkage refused.
    Reference = 0x3003,
    /// Registry cache, Loader-backed current deployment, or release-set join refused.
    Release = 0x3004,
    /// Core Market PDA, owner, width, phase, or generation refused.
    Market = 0x3005,
    /// RentCredit owner, bytes, PDA, or persisted beneficiary refused.
    RentCredit = 0x3006,
    /// System, Rent, Clock, vacant account, or exact creation plan refused.
    Creation = 0x3007,
    /// Capability manifest entry, funding ledger, custody, deadline, or PDA refused.
    Funding = 0x3008,
    /// Canonical release-pinned Core caller authority refused.
    CallerAuthority = 0x3009,
    /// Selected child invocation or immediate return-data producer refused.
    ChildCpi = 0x300A,
    /// Child acknowledgement or post-funding physical delta refused.
    ChildAck = 0x300B,
    /// Generated semantic transition refused.
    Transition = 0x300C,
    /// Commit-last Core state persistence postcheck refused.
    Commit = 0x300D,
    /// Checked arithmetic or bounded conversion refused.
    Arithmetic = 0x300E,
    /// Core bootstrap profile, artifact, Loader, or immutability authority refused.
    Infrastructure = 0x300F,
    /// The release's pinned deployment slot moved: the substrate was upgraded.
    /// Every open market on the superseded release generation refuses until a
    /// re-release re-authenticates the new deployment and re-pins its slot.
    ///
    /// Decision 0012. Not a corrupted account and not an attack: the exact
    /// upgrade authority the release names shipped new bytes, so the cached
    /// authentication no longer describes what is deployed.
    ReleaseSuperseded = 0x3010,
    /// A Source material bought a recovery walk that no live route can walk.
    ///
    /// Liveness census R2/Q2 (`docs/evidence/LIVENESS_CENSUS_2026_08_29.md`).
    /// `SourceResolutionStateV2::exhaust_after_primary_deadline` refuses any
    /// material carrying a recovery policy
    /// (`source_resolution_v2.rs`, `Error::RecoveryNotExhausted`), and the
    /// ordered ladder that was supposed to consume those paid-for legs has no
    /// live call site — `funded::process_funded_transition` is reachable only
    /// from a `#[cfg(any())]` function. So a resolution fund created over such
    /// a material admits neither the success capture nor the failure walk at
    /// its deadline: it has no terminal at all, and every holder's principal
    /// stays in it forever.
    ///
    /// `CreateFund` is therefore refused for a recovery-policy material. This
    /// is a weld, not a design: it refuses to *create* the un-terminalizable
    /// resolution state. `VerifyFundReady` is deliberately untouched, so any
    /// state that already exists keeps every route it has. (`CloseFund` was
    /// named here too until V7 moved the close out of Core entirely; it now
    /// earns [`CoreSbfError::UnsupportedAction`] at decode, weld or no weld.)
    /// The weld lifts when the ladder gets a live route.
    RecoveryWalkUnavailable = 0x3011,
    /// A basis declaring degree >= 2 was founded with no `DCLTPGT1` price-gate
    /// certificate account offered.
    ///
    /// Degree <= 1 is exempt from the no-arbitrage gate **by proof**: at that
    /// degree the simplex condition is still the whole no-arbitrage condition.
    /// Above it that stops being true, so founding a curved basis without a
    /// certificate would admit an executable arbitrage.
    PriceGateRequired = 0x3012,
    /// The certificate account offered was not the one the authenticated basis
    /// record names.
    ///
    /// The digest is read off the basis, never off the caller, so this covers
    /// a wrong account, a **byte-identical certificate at a non-canonical
    /// address**, a Registry-unowned or writable account, and one below rent
    /// exemption for its exact 320-byte width.
    PriceGateBasisMismatch = 0x3013,
    /// **The hull identity failed.** `price * mass != sum(weight * payout)` at
    /// some claim, with every payout recomputed through the production
    /// evaluator rather than read from the certificate. This is the refusal a
    /// forged certificate earns.
    PriceGateHullRefused = 0x3014,
    /// The certificate carried no hull atoms, or more than the
    /// affine-Caratheodory capacity of ten permits.
    PriceGateCapacity = 0x3015,
    /// The certificate's body was non-canonical: padding past a declared
    /// width, coordinates not strictly increasing, a zero atom weight, a
    /// non-primitive weight scale, or prices not partitioning the scale.
    PriceGateNonCanonical = 0x3016,
    /// The succession ceremony found no decodable V1 profile at its PDA.
    ///
    /// Succession without a predecessor is initialization's job
    /// (`InitializeProtocolInfrastructureV1`); the V2 ceremony refuses a
    /// vacant or malformed V1 by name (ruling §5 conjunct 2).
    InfrastructurePredecessorAbsent = 0x3017,
    /// A succession tried to move the Registry or Rent program identity.
    ///
    /// A hop may move a role's bytes, never its identity (the lineage
    /// machinery's conjunct 4, applied to the infrastructure pair). A
    /// program-id move is a different, bigger act — refused here by name,
    /// always (ruling §5 conjunct 3).
    InfrastructureIdentityMoved = 0x3018,
    /// The succession does not move strictly forward.
    ///
    /// A moved binding's successor record must bind a strictly later
    /// deployment slot than its predecessor record (Loader V3 slots only
    /// move forward), and a succession in which NEITHER binding moves
    /// selects nothing new and would only burn the one V2 vacancy
    /// (ruling §5 conjunct 4; the lineage self-succession refusal).
    InfrastructureNotForward = 0x3019,
    /// A moved binding lacks its predecessor release's bound authority.
    ///
    /// The key the Loader already required for the physical upgrade must
    /// co-sign the re-selection; an unmoved binding must carry the System
    /// program — no consent, and nothing that could look like consent —
    /// in its consent slot (ruling §5 conjunct 5).
    InfrastructureConsentMissing = 0x301A,
    /// The V2 profile PDA is already occupied: the succession happened.
    ///
    /// Write-once by the same vacancy discipline as V1 — one succession
    /// per domain, ever. A second ceremony is a fork attempt and refuses
    /// by name (ruling §5 conjunct 6).
    InfrastructureAlreadySucceeded = 0x301B,
    /// A wire action this program decodes and no longer composes.
    ///
    /// The action is well-formed and its discriminant is still live on the
    /// wire — another program dispatches it — so `Instruction` would be the
    /// wrong accusation: nothing about the caller's bytes is malformed. What
    /// is true is narrower and is what a reader needs: *Core is not the owner
    /// of this route any more*.
    ///
    /// Today exactly one action earns it. `ResolutionCoreActionV1::CloseFund`
    /// used to close the Source subtree through a Core-composed child
    /// invocation; V7 moved that to `process_direct_funding_close_v1` in the
    /// Resolution program because the composed route authenticated the same
    /// plan on both sides of the CPI and exceeded the transaction compute
    /// ceiling. Core refuses it at decode, before any authentication work is
    /// spent on an instruction that cannot succeed.
    UnsupportedAction = 0x301C,
    /// The rent a funding ledger was FUNDED at did not price its balance.
    ///
    /// Split from `Funding` on 2026-09-04. `Funding` covered every conjunct of
    /// the custody arithmetic, including the one term a reader cannot see from
    /// the account: the exemption-scaled rent rate the ledger's header records.
    /// A cluster that changes its rent-exempt rate under a live cohort refuses
    /// here and nowhere else, and this code says so instead of naming the
    /// whole of funding.
    FundedRent = 0x301D,
}

impl CoreSbfError {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`CoreSbfError::ordinal`], whose match is exhaustive: a variant added to the
    /// enum does not compile until its author writes an arm here, and the only
    /// arm that satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 30] = [
        Self::Instruction,
        Self::AccountFrame,
        Self::FinalizedRecord,
        Self::Reference,
        Self::Release,
        Self::Market,
        Self::RentCredit,
        Self::Creation,
        Self::Funding,
        Self::CallerAuthority,
        Self::ChildCpi,
        Self::ChildAck,
        Self::Transition,
        Self::Commit,
        Self::Arithmetic,
        Self::Infrastructure,
        Self::ReleaseSuperseded,
        Self::RecoveryWalkUnavailable,
        Self::PriceGateRequired,
        Self::PriceGateBasisMismatch,
        Self::PriceGateHullRefused,
        Self::PriceGateCapacity,
        Self::PriceGateNonCanonical,
        Self::InfrastructurePredecessorAbsent,
        Self::InfrastructureIdentityMoved,
        Self::InfrastructureNotForward,
        Self::InfrastructureConsentMissing,
        Self::InfrastructureAlreadySucceeded,
        Self::UnsupportedAction,
        Self::FundedRent,
    ];

    /// This refusal's position in [`CoreSbfError::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism:
    /// a thirtieth variant is a COMPILE ERROR here rather than a discriminant no
    /// assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::AccountFrame => 1,
            Self::FinalizedRecord => 2,
            Self::Reference => 3,
            Self::Release => 4,
            Self::Market => 5,
            Self::RentCredit => 6,
            Self::Creation => 7,
            Self::Funding => 8,
            Self::CallerAuthority => 9,
            Self::ChildCpi => 10,
            Self::ChildAck => 11,
            Self::Transition => 12,
            Self::Commit => 13,
            Self::Arithmetic => 14,
            Self::Infrastructure => 15,
            Self::ReleaseSuperseded => 16,
            Self::RecoveryWalkUnavailable => 17,
            Self::PriceGateRequired => 18,
            Self::PriceGateBasisMismatch => 19,
            Self::PriceGateHullRefused => 20,
            Self::PriceGateCapacity => 21,
            Self::PriceGateNonCanonical => 22,
            Self::InfrastructurePredecessorAbsent => 23,
            Self::InfrastructureIdentityMoved => 24,
            Self::InfrastructureNotForward => 25,
            Self::InfrastructureConsentMissing => 26,
            Self::InfrastructureAlreadySucceeded => 27,
            Self::UnsupportedAction => 28,
            Self::FundedRent => 29,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing
// about the variants after it and goes stale silently every single time the
// enum grows -- the failure is not that the name is wrong, it is that nothing
// can notice. Claims proved it the expensive way: its bound went on naming
// `ReleaseSuperseded` after a later variant landed, so for as long as that
// stood, the newest refusal in the program was checked by nothing.
//
// So the band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    assert!(
        CoreSbfError::ALL[0] as u32 == dclutch_refusal_registry::CORE_REFUSAL_BASE,
        "CoreSbfError must start at its registered refusal band base"
    );
    let mut index: u32 = 0;
    let mut rest = CoreSbfError::ALL.as_slice();
    while let [variant, tail @ ..] = rest {
        let variant = *variant;
        assert!(
            variant.ordinal() == index as usize,
            "CoreSbfError::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == dclutch_refusal_registry::CORE_REFUSAL_BASE + index,
            "CoreSbfError discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::CORE_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
            "CoreSbfError must not run past its registered refusal band"
        );
        index += 1;
        rest = tail;
    }
};

impl From<CoreSbfError> for ProgramError {
    fn from(value: CoreSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

/// Name an activation-cache refusal, keeping the superseded case actionable.
///
/// Decision 0012: a moved deployment slot means the substrate was upgraded and
/// this release generation is finished. The remedy is a re-release, not an
/// investigation, so it does not fold into the generic Release refusal.
impl From<dclutch_registry_activation_auth_v1::ActivationAuthErrorV1> for CoreSbfError {
    fn from(value: dclutch_registry_activation_auth_v1::ActivationAuthErrorV1) -> Self {
        match value {
            dclutch_registry_activation_auth_v1::ActivationAuthErrorV1::ReleaseSuperseded => {
                Self::ReleaseSuperseded
            }
            _ => Self::Release,
        }
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Execute one supported sparse Core transition.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() >= AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1
        && matches!(
            instruction_data.get(..8),
            Some(magic)
                if magic == AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1
                    || magic == AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1
                    || magic == AGGREGATE_RETIREMENT_FINISH_MAGIC_V1
        )
    {
        return retire_v1::process_checkpoint_suffix(program_id, accounts, instruction_data);
    }
    if instruction_data.len()
        == dclutch_custody_contract::RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1
        && instruction_data
            .get(..dclutch_custody_contract::RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1.len())
            == Some(dclutch_custody_contract::RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1.as_slice())
    {
        let request =
            dclutch_custody_contract::RetirementReplayHandoffRequestV1::decode(instruction_data)
                .map_err(|_| CoreSbfError::Instruction)?;
        return retirement_replay_handoff_v1::process(
            program_id,
            accounts,
            request,
            instruction_data,
        );
    }
    if instruction_data.len() == INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1 {
        // The two ceremonies share one fixed width; the magic is the whole
        // discriminant. The V2 arm is checked first so its instruction never
        // falls through to V1's decoder and dies as a generic magic refusal.
        if instruction_data.get(..INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V2.len())
            == Some(INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_V2.as_slice())
        {
            InitializeProtocolInfrastructureV2::decode(instruction_data)
                .map_err(|_| CoreSbfError::Instruction)?;
            return infrastructure_v2::process_initialize_v2(program_id, accounts);
        }
        InitializeProtocolInfrastructureV1::decode(instruction_data)
            .map_err(|_| CoreSbfError::Instruction)?;
        return infrastructure::process_initialize(program_id, accounts);
    }
    const _: () = assert!(
        INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1 == INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V2,
        "the length dispatch above serves both ceremony versions"
    );
    if instruction_data.len() >= SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1
        && instruction_data.get(..SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.len())
            == Some(SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let proof_bytes = instruction_data
            .get(SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1..)
            .ok_or(CoreSbfError::Instruction)?;
        if proof_bytes.len() % 32 != 0 {
            return Err(CoreSbfError::Instruction.into());
        }
        let request = SeriesUnallocatedPermitExpiryRequestV1::decode(request_bytes)
            .map_err(|_| CoreSbfError::Instruction)?;
        return series_permit_expiry_precommit_v1::process(
            program_id,
            accounts,
            request,
            request_bytes,
            proof_bytes,
        );
    }
    if instruction_data.len() >= SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1
        && instruction_data.get(..SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1.len())
            == Some(SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let proof_bytes = instruction_data
            .get(SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1..)
            .ok_or(CoreSbfError::Instruction)?;
        let request = SeriesPermitExpiryRequestV1::decode(request_bytes)
            .map_err(|_| CoreSbfError::Instruction)?;
        return series_permit_expiry::process(program_id, accounts, request, proof_bytes);
    }
    if instruction_data.len() >= GENERIC_FOUNDING_REQUEST_BYTES_V1
        && instruction_data.get(..GENERIC_FOUNDING_REQUEST_MAGIC_V1.len())
            == Some(GENERIC_FOUNDING_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..GENERIC_FOUNDING_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let dependency_bytes = instruction_data
            .get(GENERIC_FOUNDING_REQUEST_BYTES_V1..)
            .ok_or(CoreSbfError::Instruction)?;
        let request = GenericFoundingRequestV1::decode(request_bytes)
            .map_err(|_| CoreSbfError::Instruction)?;
        return generic_founding_v1::process(
            program_id,
            accounts,
            request,
            request_bytes,
            dependency_bytes,
        );
    }
    if instruction_data.len() >= SERIES_CORE_REQUEST_BYTES_V1
        && instruction_data.get(..SERIES_CORE_REQUEST_MAGIC_V1.len())
            == Some(SERIES_CORE_REQUEST_MAGIC_V1.as_slice())
    {
        let request_bytes = instruction_data
            .get(..SERIES_CORE_REQUEST_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?;
        let request =
            SeriesCoreRequestV1::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
        if let Some(dependency_start) = instruction_data
            .len()
            .checked_sub(CLAIMS_FOUNDING_RECEIPT_BYTES_V5)
            .filter(|start| *start >= SERIES_CORE_REQUEST_BYTES_V1)
        {
            let claims_receipt_bytes = instruction_data
                .get(dependency_start..)
                .ok_or(CoreSbfError::Instruction)?;
            if claims_receipt_bytes.get(..CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.len())
                == Some(CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.as_slice())
            {
                let proof_bytes = instruction_data
                    .get(SERIES_CORE_REQUEST_BYTES_V1..dependency_start)
                    .ok_or(CoreSbfError::Instruction)?;
                return series_open::process(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    proof_bytes,
                    claims_receipt_bytes,
                );
            }
        }
        if let Some(dependency_start) = instruction_data
            .len()
            .checked_sub(PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1)
            .filter(|start| *start >= SERIES_CORE_REQUEST_BYTES_V1)
        {
            let lock_receipt_bytes = instruction_data
                .get(dependency_start..)
                .ok_or(CoreSbfError::Instruction)?;
            if lock_receipt_bytes.get(..PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1.len())
                == Some(PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1.as_slice())
            {
                let proof_bytes = instruction_data
                    .get(SERIES_CORE_REQUEST_BYTES_V1..dependency_start)
                    .ok_or(CoreSbfError::Instruction)?;
                return series_consume::process(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    proof_bytes,
                    lock_receipt_bytes,
                );
            }
        }
        return Err(CoreSbfError::Instruction.into());
    }
    if instruction_data.len() == PROJECT_FOUND_REQUEST_BYTES_V2
        && instruction_data.get(..PROJECT_FOUND_REQUEST_MAGIC_V2.len())
            == Some(PROJECT_FOUND_REQUEST_MAGIC_V2.as_slice())
    {
        let projected = ProjectFoundRequestV2::decode(instruction_data)
            .map_err(|_| CoreSbfError::Instruction)?;
        let found_bytes = projected
            .found
            .encode()
            .map_err(|_| CoreSbfError::Instruction)?;
        return found::project(program_id, accounts, projected.found, &found_bytes);
    }
    let request_bytes = instruction_data
        .get(..REQUEST_BYTES)
        .ok_or(CoreSbfError::Instruction)?;
    // **The `Action` family's magic, read at the dispatch and not only inside
    // the decoder.** `Request::decode` has always checked it -- `exact_magic`
    // is unchanged and still runs, and it is the codec's own hostile check --
    // but a magic no dispatch guard names is a magic the route census cannot
    // attribute to a route: its walk treats the decode as terminal. So
    // `DCLTCRQ2` selected no route at all while every act driving one of the
    // arms below carried it on the wire, and `corroborate.py --discover`
    // dropped every signature it resolved to Core across three cohorts. This
    // is the shape the Series arm above already uses for `DCLTCSR1`, where the
    // dispatch and `SeriesCoreRequestV1::decode` also both check it.
    if request_bytes.get(..CORE_REQUEST_MAGIC.len()) == Some(CORE_REQUEST_MAGIC.as_slice()) {
        let request = Request::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
        return match request.action {
            Action::Found if instruction_data.len() == REQUEST_BYTES => {
                found::process(program_id, accounts, request)
            }
            Action::BeginRetiring if instruction_data.len() == REQUEST_BYTES => {
                begin_retiring::process(program_id, accounts, request)
            }
            Action::ExecuteProvider
                if instruction_data.len()
                    > execute_provider_v3::EXECUTE_PROVIDER_PREFIX_BYTES_V3 =>
            {
                let provider_data = instruction_data
                    .get(REQUEST_BYTES..)
                    .ok_or(CoreSbfError::Instruction)?;
                execute_provider_v3::process(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    provider_data,
                )
            }
            Action::OpenMarket
                if instruction_data.len() == open_market::OPEN_MARKET_INSTRUCTION_BYTES_V1 =>
            {
                let custody_bytes = instruction_data
                    .get(REQUEST_BYTES..)
                    .ok_or(CoreSbfError::Instruction)?;
                open_market::process(program_id, accounts, request, request_bytes, custody_bytes)
            }
            Action::Retire
                if instruction_data.len() == retire_v1::RETIREMENT_INSTRUCTION_BYTES_V1 =>
            {
                let bundle_start = REQUEST_BYTES;
                let claims_start = bundle_start + RETIREMENT_BUNDLE_BYTES_V1;
                let close_vault_start = claims_start + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
                let close_replay_start = close_vault_start + CUSTODY_REQUEST_BYTES_V1;
                let bundle_bytes = instruction_data
                    .get(bundle_start..claims_start)
                    .ok_or(CoreSbfError::Instruction)?;
                let claims_request_bytes = instruction_data
                    .get(claims_start..close_vault_start)
                    .ok_or(CoreSbfError::Instruction)?;
                let close_vault_request_bytes = instruction_data
                    .get(close_vault_start..close_replay_start)
                    .ok_or(CoreSbfError::Instruction)?;
                let close_replay_request_bytes = instruction_data
                    .get(close_replay_start..)
                    .ok_or(CoreSbfError::Instruction)?;
                retire_v1::process(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    bundle_bytes,
                    claims_request_bytes,
                    close_vault_request_bytes,
                    close_replay_request_bytes,
                )
            }
            Action::Retire
                if instruction_data.len()
                    == retire_v1::RETIREMENT_CHECKPOINT_PREPARE_INSTRUCTION_BYTES_V1 =>
            {
                let bundle_start = REQUEST_BYTES;
                let claims_start = bundle_start + RETIREMENT_BUNDLE_BYTES_V1;
                let bundle_bytes = instruction_data
                    .get(bundle_start..claims_start)
                    .ok_or(CoreSbfError::Instruction)?;
                let claims_request_bytes = instruction_data
                    .get(claims_start..)
                    .ok_or(CoreSbfError::Instruction)?;
                if claims_request_bytes.get(
                    ..dclutch_claims_svm::retirement_checkpoint_handoff_v1::CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_MAGIC_V1.len(),
                ) != Some(
                    dclutch_claims_svm::retirement_checkpoint_handoff_v1::CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_MAGIC_V1.as_slice(),
                ) {
                    return Err(CoreSbfError::Instruction.into());
                }
                retire_v1::process_checkpoint_prepare(
                    program_id,
                    accounts,
                    request,
                    request_bytes,
                    bundle_bytes,
                    claims_request_bytes,
                )
            }
            Action::ActivateCapability | Action::CloseCapability => {
                let envelope_end = CAPABILITY_PREFIX_BYTES_V1;
                let envelope_bytes = instruction_data
                    .get(REQUEST_BYTES..envelope_end)
                    .ok_or(CoreSbfError::Instruction)?;
                let role_request = instruction_data
                    .get(envelope_end..)
                    .ok_or(CoreSbfError::Instruction)?;
                let selection_bytes = role_request
                    .get(..CAPABILITY_EXECUTION_SELECTION_BYTES_V1)
                    .ok_or(CoreSbfError::Instruction)?;
                let header_end = CAPABILITY_ROLE_PREFIX_BYTES_V2;
                let header_bytes = role_request
                    .get(CAPABILITY_EXECUTION_SELECTION_BYTES_V1..header_end)
                    .ok_or(CoreSbfError::Instruction)?;
                let family_request = role_request
                    .get(header_end..)
                    .ok_or(CoreSbfError::Instruction)?;
                if family_request.is_empty() {
                    return Err(CoreSbfError::Instruction.into());
                }
                let envelope = CoreEffectEnvelopeV1::decode(envelope_bytes)
                    .map_err(|_| CoreSbfError::Instruction)?;
                let selection = CapabilityExecutionSelectionV1::decode(selection_bytes)
                    .map_err(|_| CoreSbfError::Instruction)?;
                let funding_header = CapabilityFundingHeaderV2::decode(header_bytes)
                    .map_err(|_| CoreSbfError::Instruction)?;
                capability::process(
                    program_id,
                    accounts,
                    request,
                    envelope,
                    envelope_bytes,
                    role_request,
                    selection,
                    funding_header,
                )
            }
            Action::VerifyReadiness | Action::AdmitTerminal | Action::Retire
                if instruction_data.len() == resolution::RESOLUTION_CORE_INSTRUCTION_BYTES_V1 =>
            {
                let envelope_end = CAPABILITY_PREFIX_BYTES_V1;
                let envelope_bytes = instruction_data
                    .get(REQUEST_BYTES..envelope_end)
                    .ok_or(CoreSbfError::Instruction)?;
                let role_request = instruction_data
                    .get(envelope_end..)
                    .ok_or(CoreSbfError::Instruction)?;
                let envelope = CoreEffectEnvelopeV1::decode(envelope_bytes)
                    .map_err(|_| CoreSbfError::Instruction)?;
                resolution::process(
                    program_id,
                    accounts,
                    request,
                    envelope,
                    envelope_bytes,
                    role_request,
                )
            }
            _ => Err(CoreSbfError::Instruction.into()),
        };
    }
    Err(CoreSbfError::Instruction.into())
}

#[cfg(test)]
mod tests;
