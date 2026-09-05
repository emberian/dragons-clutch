#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Registry-bound Core-effect Source Resolution controller.

extern crate alloc;
extern crate std;

use dclutch_capability_contract::CapabilityManifestV1;
use dclutch_capability_contract::funding::funded_rent_persists_v1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_activation_auth_v1::{
    ActivationAuthErrorV1, cached_role_deployment_observation_v1,
};
use dclutch_registry_contract::{ArtifactReleaseV1, DeploymentObservationV1};
use dclutch_source_contract::{RecoveryPolicyV2, SourceMaterialV3};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, hash::hash,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

mod core_effect;
/// Current-ABI funded liveness-walk accounting: the escrowed explicit-failure
/// compartment a deadline-driven terminal spends.
pub mod funded;
mod market_admission_v1;
mod pre_market_funding_abort_v1;
mod pre_market_funding_v1;
mod provider_instruction_v3;
mod provider_transport_v3;
/// Current-ABI real-provider evidence composition shared by fixed Core and
/// data-defined Trading callers.
pub mod provider_v3;
mod relay_transport_v1;
/// Current-ABI sealed relayed-record evidence composition.
pub mod relay_v1;
mod sponsored_push_v1;

/// Stable Resolution controller refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionError {
    /// Account count, order, privilege, executable state, or aliasing was invalid.
    AccountFrame = 0x8000,
    /// The generated fixed-layout request refused hostile bytes.
    Instruction = 0x8001,
    /// A writable Source state or certificate account was not canonical.
    OutputState = 0x8002,
    /// Market owner, root, lifecycle, generation, or Source binding was invalid.
    MarketAuthority = 0x8003,
    /// A finalized raw-record owner, PDA, digest, rent, or vacancy proof was invalid.
    FinalizedRecord = 0x8004,
    /// The Market-selected Registry activation did not authorize this Resolution release.
    ResolutionRelease = 0x8005,
    /// Current Loader V3 Program, ProgramData, ELF, slot, or upgrade policy was substituted.
    ResolutionDeployment = 0x8006,
    /// Source material or one of its embedded content identities was inconsistent.
    SourceMaterial = 0x8007,
    /// The external Product-owned result-domain identity or bytes differed.
    ProductDomain = 0x8008,
    /// The selected Pyth provider-release record or Loader accounts differed.
    ProviderRelease = 0x8009,
    /// Fully verified update authentication failed: the posted bytes, their
    /// digest, the write authority, the posted slot, or an evidence identity
    /// was not the one this frame committed to.
    ///
    /// This used to be all of §12.3 as well. It is not any more: the three
    /// questions `docs/design/MAINNET_STATE_RELAY.md` §12.3 says an operator
    /// must be able to tell apart now have their own codes
    /// (`ProviderWindow`, `ProviderFreshness`, `ProviderConfiguration`), and
    /// this one is the residue — "the update itself did not authenticate".
    ProviderObservation = 0x800A,
    /// Clock or Rent sysvar identity or bytes were invalid.
    Sysvar = 0x800B,
    /// Provider-neutral Source admission or Product mapping refused.
    Transition = 0x800C,
    /// Checked physical arithmetic or signed timestamp conversion failed.
    Arithmetic = 0x800D,
    /// Canonical capability funding, typed custody, or exact bounty debit failed.
    Funding = 0x800E,
    /// The sealed relayed observation record was not consumable against this
    /// Market's authenticated Source graph.
    RelayedRecord = 0x800F,
    /// The relayed observation was admissible but did not satisfy the Product's
    /// own window: it is no answer rather than a wrong one, and the market is
    /// still live. Distinct from every "the bytes were wrong" refusal on
    /// purpose, because "come back later" and "something is broken" are not the
    /// same message to whoever is holding the position.
    RelayedWindow = 0x8010,
    /// The provider's observation is not ABOUT the period this Market sold:
    /// its publication time is outside `[window.start, window.end]`.
    ///
    /// Like `RelayedWindow`, and for the same reason: this is no answer rather
    /// than a wrong one, and the Market is still live. §12.3's first operator
    /// question.
    ProviderWindow = 0x8011,
    /// The provider's observation is about the right period and this cluster
    /// will not act on it: its publication time is outside
    /// `[now - max_age, now + max_future_skew]`.
    ///
    /// §12.3's second operator question, and the one whose answer is an
    /// instruction: if the publication is too OLD, a pinned fixture has
    /// outlived its declared shelf life and must be recaptured — not widened.
    ProviderFreshness = 0x8012,
    /// The provider's observation is timely and about the right period, and its
    /// feed identity, exponent, or confidence is not what this Market's adapter
    /// configuration admits.
    ///
    /// §12.3's third operator question. Unlike the first two this one is not
    /// "come back later": nothing about waiting changes it.
    ProviderConfiguration = 0x8013,
    /// The release's pinned deployment slot moved: the substrate was upgraded.
    /// Every open market on the superseded release generation refuses until a
    /// re-release re-authenticates the new deployment and re-pins its slot.
    ///
    /// Decision 0012. Not a corrupted account and not an attack: the exact
    /// upgrade authority the release names shipped new bytes, so the cached
    /// authentication no longer describes what is deployed.
    ReleaseSuperseded = 0x8014,
    /// Sponsored-push candidate, head, release, or deadline authentication failed.
    SponsoredPush = 0x8015,
    /// `RetireRecord` was aimed at evidence a still-live market could consume.
    ///
    /// Liveness census Y3 / queue Q9. `RetireRecord` is permissionless and it
    /// CLOSES the account, so before this code existed anyone could delete a
    /// fully sealed quorum observation for a transaction fee and force the
    /// market onto the failure walk — where the walker collects a bounty and
    /// the holders get the pre-disclosed failure outcome instead of the real
    /// one. Retiring evidence that is not yet `Consumed` now requires the
    /// Market to carry a terminal receipt.
    ///
    /// This is "not yet", not "never": consumption is itself permissionless,
    /// the funded failure walk terminalizes the market with no identified
    /// party's help, and once `terminal_receipt` is `Some` every phase retires
    /// exactly as it always did. No rent is stranded, only deferred.
    RecordStillConsumable = 0x8016,
    /// `AbandonSubmission` was aimed at a submission a Source could still consume.
    ///
    /// The mirror of [`ResolutionError::RecordStillConsumable`] on the provider
    /// transport, and it exists for the same reason: closing evidence that is
    /// still live would let anyone delete a market's answer for a transaction
    /// fee. Abandonment requires BOTH that the submitter's own
    /// `reclaim_after_unix_seconds` has passed AND that the Source can no
    /// longer consume this update — it has left `Primary`, or its account has
    /// already been discharged by `CloseFund`.
    ///
    /// It needs its own code because it is the only refusal on this route a
    /// perfectly well-formed request from an honest party can trigger, and it
    /// means "right route, wrong moment". A reader who saw `OutputState` would
    /// go hunting a malformed account that is not there.
    SubmissionStillConsumable = 0x8017,
    /// The account offered as this Market's activation is not the canonical
    /// Registry-owned cache for the release set the frame names.
    ///
    /// Owner, executable state, width, borrow, decode, the cache's own
    /// `execution_release_set_id`, its equality with the release set the
    /// request or Core-effect envelope names, and the
    /// `ACTIVATION_PDA_DOMAIN_V1` address under the Registry in the frame.
    /// Every one of these is answered before a single role is projected, and
    /// what they say together is "this is not the activation you named" --
    /// which is a different accusation from "the activation named a different
    /// program for a role", and used to be the same code as it.
    ActivationCache = 0x8018,
    /// The activation is canonical, and the program brought for a role OTHER
    /// than Resolution is not the one that activation selected for it.
    ///
    /// Core, Trading, Claims or Custody: the role projection is missing from
    /// the cache, or the program account standing in that slot is not the one
    /// the release set binds. This is the conjunct a caller hits when it
    /// executes against a release set whose roles it did not actually deploy,
    /// and it is deliberately NOT
    /// [`ResolutionError::ResolutionRelease`] -- that one is about this
    /// program's own release, and a reader who cannot tell them apart cannot
    /// tell "you brought the wrong Trading program" from "you are running the
    /// wrong Resolution controller".
    ActivatedRole = 0x8019,
    /// The calling role's authority PDA is not the one the frame's own seeds
    /// derive.
    ///
    /// The `CallerAuthoritySeedsV1` construction, the account the caller
    /// offered as its authority, and the caller-program/parent-digest identity
    /// the seeds are built from. It means the composition is unauthenticated
    /// at the CALLER, not that any release was wrong.
    CallerAuthority = 0x801A,
    /// The Core-owned protocol infrastructure profile, or the Registry release
    /// it names, did not authenticate.
    ///
    /// The profile's PDA, owner, width and rent exemption, its decode, the
    /// Registry program it names, and the `ArtifactReleaseV1` reached through
    /// it including its slot pin. This is upstream of every activation
    /// question: it is how a frame learns WHICH Registry to believe, so a
    /// failure here means the frame never got as far as a release set.
    InfrastructureProfile = 0x801B,
    /// This market's own StatisticSpec and adapter configuration disagree
    /// about the source-to-result decimal scale.
    ///
    /// Reached only after the publication itself was admitted, so it is never
    /// a complaint about the provider: the feed published exactly what this
    /// market pinned and the market's two records still do not agree about
    /// what the number means. No publication can satisfy such a market, which
    /// is why it is not `ProviderConfiguration` -- an operator seeing this
    /// should stop resubmitting and read the founding.
    ProviderScale = 0x801C,
    /// The rung this capture names is not the rung the market stands on.
    ///
    /// A funded ordered-recovery ladder answers on exactly one source at a
    /// time, and which one is the Source state's own `active_attempt` -- never
    /// the caller's `source_index`, which is a declaration this refusal
    /// enforces. It fires when a capture claims the primary while the market
    /// has advanced, claims a rung while the market is still on its primary,
    /// claims a rung the market has already left or has not reached, or brings
    /// no `RecoveryPolicyV2` to a market whose ladder is the only thing that
    /// could name the source it is offering. It also covers a market standing
    /// on NO leg -- one already `Resolved`, `Exhausted`, `FailureCommitted` or
    /// `Retired` -- because there is then no index a capture could name that
    /// would be right.
    ///
    /// It is deliberately not `SourceMaterial`: the graph is fine and the
    /// records authenticate. It is deliberately not `Transition` either, which
    /// would mean the kernel refused -- this refuses BEFORE the kernel, because
    /// a request that names the wrong rung would otherwise be joined against
    /// the wrong source and be refused for a reason about the feed. An operator
    /// reading this should re-read the market's phase, not the provider's
    /// bytes.
    SourceLadder = 0x801D,
}

dclutch_refusal_registry::pin_refusal_band!(
    ResolutionError,
    dclutch_refusal_registry::RESOLUTION_REFUSAL_BASE,
    [
        AccountFrame,
        Instruction,
        OutputState,
        MarketAuthority,
        FinalizedRecord,
        ResolutionRelease,
        ResolutionDeployment,
        SourceMaterial,
        ProductDomain,
        ProviderRelease,
        ProviderObservation,
        Sysvar,
        Transition,
        Arithmetic,
        Funding,
        RelayedRecord,
        RelayedWindow,
        ProviderWindow,
        ProviderFreshness,
        ProviderConfiguration,
        ReleaseSuperseded,
        SponsoredPush,
        RecordStillConsumable,
        SubmissionStillConsumable,
        ActivationCache,
        ActivatedRole,
        CallerAuthority,
        InfrastructureProfile,
        ProviderScale,
        SourceLadder
    ]
);

pub(crate) enum RecordKind {
    CapabilityManifest,
    SourceMaterialV3,
    RecoveryPolicyV2,
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate one exact Resolution frame and atomically persist its outputs.
///
/// Direct funded transitions return the canonical funded-transition receipt
/// only after Source, certificate, FundingState, and worker payout commit.
/// Core-effect routes retain their sole canonical Core acknowledgment wire.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if pre_market_funding_abort_v1::is_pre_market_funding_abort_v1(instruction_data) {
        return pre_market_funding_abort_v1::process_pre_market_funding_abort_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if pre_market_funding_v1::is_pre_market_funding_v2(instruction_data) {
        return pre_market_funding_v1::process_pre_market_funding_v2(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if core_effect::is_direct_funding_activation_v1(instruction_data) {
        return core_effect::process_direct_funding_activation_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if core_effect::is_direct_funding_close_v1(instruction_data) {
        return core_effect::process_direct_funding_close_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if core_effect::is_core_effect(instruction_data) {
        return core_effect::process_core_effect(program_id, accounts, instruction_data);
    }
    if provider_instruction_v3::is_provider_resolution_v3(instruction_data) {
        return provider_instruction_v3::process_provider_resolution_v3(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if provider_transport_v3::is_provider_transport_v3(instruction_data) {
        return provider_transport_v3::process_provider_transport_v3(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if relay_transport_v1::is_relay_transport_v1(instruction_data) {
        return relay_transport_v1::process_relay_transport_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if sponsored_push_v1::is_sponsored_push_v1(instruction_data) {
        return sponsored_push_v1::process_sponsored_push_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    Err(ResolutionError::Instruction.into())
}

pub(crate) fn authenticate_clock(account: &AccountInfo<'_>) -> Result<Clock, ProgramError> {
    if account.key != &sysvar::clock::ID || account.owner != &sysvar::ID {
        return Err(ResolutionError::Sysvar.into());
    }
    Clock::from_account_info(account).map_err(|_| ResolutionError::Sysvar.into())
}

pub(crate) fn authenticate_rent(account: &AccountInfo<'_>) -> Result<Rent, ProgramError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID {
        return Err(ResolutionError::Sysvar.into());
    }
    Rent::from_account_info(account).map_err(|_| ResolutionError::Sysvar.into())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_finalized_record(
    core_program: Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema_id: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &[u8],
    kind: RecordKind,
) -> ProgramResult {
    if raw.owner != &core_program
        || raw.executable
        || hash(bytes).to_bytes() != expected_digest
        || !funded_rent_persists_v1(raw.lamports())
    {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema_id, &expected_digest],
        &core_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema_id, &expected_digest],
        &core_program,
    )
    .0;
    if raw.key != &expected_raw
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.lamports() != 0
        || staging.data_len() != 0
        || staging.executable
    {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    let valid = match kind {
        RecordKind::CapabilityManifest => CapabilityManifestV1::decode(bytes).is_ok(),
        RecordKind::SourceMaterialV3 => SourceMaterialV3::decode(bytes).is_ok(),
        RecordKind::RecoveryPolicyV2 => RecoveryPolicyV2::decode(bytes).is_ok(),
    };
    if !valid {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Name a pinned-deployment refusal, keeping the superseded case operator-legible.
///
/// Decision 0012: a moved deployment slot is the expected consequence of
/// upgrading the substrate, not a corrupted account, and its remedy is a
/// re-release rather than an investigation. Every other reason folds into
/// `ResolutionDeployment` exactly as before.
pub(crate) const fn pinned_deployment_refusal(
    error: dclutch_registry_contract::Error,
) -> ResolutionError {
    match error {
        dclutch_registry_contract::Error::ReleaseSupersededByUpgrade => {
            ResolutionError::ReleaseSuperseded
        }
        _ => ResolutionError::ResolutionDeployment,
    }
}

/// Re-observe one activation-pinned deployment without re-hashing its ELF.
///
/// Registry admission is the semantic owner of the full ELF digest. Recurring
/// Resolution routes still authenticate the live Loader Program and
/// ProgramData accounts, the release identities, owner/executable flags,
/// Program-to-ProgramData link, deployment slot, and upgrade authority. The
/// shared adapter reuses the admitted digest only after all of those facts
/// match, and names a moved exact-authority deployment as superseded.
pub(crate) fn cached_deployment_observation(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, ProgramError> {
    cached_role_deployment_observation_v1(program, programdata, release).map_err(|error| {
        match error {
            ActivationAuthErrorV1::ReleaseSuperseded => ResolutionError::ReleaseSuperseded,
            ActivationAuthErrorV1::AccountFrame
            | ActivationAuthErrorV1::ActivationCache
            | ActivationAuthErrorV1::Deployment => ResolutionError::ResolutionDeployment,
        }
        .into()
    })
}

#[cfg(test)]
mod tests;
