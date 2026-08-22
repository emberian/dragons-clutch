//! The pull-profile authentication join.
//!
//! Runtime implementation of `research/source-profile-v1/src/auth_v2.rs`, with
//! one deliberate strengthening: the model takes `LoaderStateV1` and
//! `ImmediatePostV1` as *inputs* and says plainly that "a caller-provided
//! struct proves nothing about instruction history". Here they are **derived**,
//! inside this function, from the pinned decoders
//! ([`crate::loader_state::decode_loader_pair_v1`] and
//! [`crate::instructions_sysvar::InstructionsSysvarV1::immediate_post_v1`]),
//! so there is no signature through which a caller could assert either
//! projection. That closes the model's named gap rather than reproducing it.
//!
//! ## What one successful call establishes
//!
//! For one ephemeral price-update account, at one canonical boundary:
//!
//! 1. the release triple compiled into this ELF matches the immutable spec;
//! 2. the receiver program is the pinned key, is executable, is loader-owned,
//!    and itself names the presented ProgramData account;
//! 3. that ProgramData account records the pinned deployment slot — so any
//!    in-place upgrade is a different source generation by construction;
//! 4. the receiver `Config` account is the pinned key, is receiver-owned, and
//!    its **complete** body digests to the pinned governance generation;
//! 5. the Clock is the canonical sysvar and the release has activated;
//! 6. the *immediately preceding* instruction in this transaction invoked the
//!    receiver program through the reviewed `post_update` discriminator,
//!    seven-account shape and effective privileges, naming this exact update
//!    account, the pinned config, and the update's own recorded write authority
//!    — adjacency read from the sysvar, never asserted;
//! 7. the update parses as a fully verified message for the pinned feed;
//! 8. its receiver-write slot and publish time are inside both freshness
//!    envelopes, measured against canonical Clock;
//! 9. the named boundary has matured past its grace delay;
//! 10. the widened confidence is inside both the absolute and relative caps;
//! 11. the update witnesses that exact boundary under `CROSSING_V1`, and the
//!     conservative interval normalizes without loss of containment.
//!
//! ## What it does not establish
//!
//! Nothing about ledger finality or RPC commitment: an executing instruction
//! cannot observe either, so there is deliberately no `finalized` bit anywhere
//! in this module. Operators must submit only after the post transaction is
//! observed at the commitment they require; the program independently enforces
//! canonical Clock and the boundary grace delay, which is a different and
//! weaker guarantee, honestly labelled.

use crate::instructions_sysvar::{InstructionsSysvarError, InstructionsSysvarV1, SYSVAR_OWNER_ID};
use crate::loader_state::{
    decode_loader_pair_v1, LoaderAccountViewV1, LoaderStateError, LoaderStateV1,
    UpgradeAuthorityV1, UPGRADEABLE_LOADER_ID,
};
use crate::pyth_receiver::{
    config_byte_digest, normalize_unsigned_ceil, parse_full_price_update_v2, FullPriceUpdateV2,
    PriceUpdateAccountViewV1, PythReceiverError,
};
use crate::source_identity::PullReleaseV2;

use super::crossing::{boundary_instant, record_from_witness, ArchiveRecordV2, CrossingError};
use super::spec::SourceSpecV2;

/// Length of the canonical Clock sysvar account body.
///
/// `slot`, `epoch_start_timestamp`, `epoch`, `leader_schedule_epoch`,
/// `unix_timestamp`, each eight bytes.
pub const CLOCK_SYSVAR_LEN: usize = 40;

/// Offset of the signed Unix timestamp inside the Clock sysvar body.
pub const CLOCK_UNIX_TIMESTAMP_OFFSET: usize = 32;

/// A metadata-bearing account view for the roles this join reads as raw bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountViewV2<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> AccountViewV2<'a> {
    /// Wrap one runtime account at the `AccountInfo` boundary.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }
}

/// Clock observation projected from the canonical sysvar account.
///
/// There is deliberately no `finalized` bit: commitment is a client and ledger
/// property that an executing instruction cannot establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockViewV1 {
    /// Address the projection was read from.
    pub key: [u8; 32],
    /// Current slot.
    pub slot: u64,
    /// Current Unix timestamp, **signed**.
    ///
    /// The rest of the runtime converts to `u64` at the sysvar boundary and
    /// refuses negatives. This profile keeps the sign because every comparison
    /// it makes is against a signed provider publish time, and converting first
    /// would turn a pre-epoch clock into a refusal in the wrong vocabulary.
    pub unix_timestamp: i64,
}

/// Capability emitted only after every identity, time, parser, and selection
/// check succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPullUpdateV2 {
    /// Canonical immutable SourceSpec v2 identity; never the ephemeral key.
    pub source_identity: [u8; 32],
    /// The exact ephemeral account the adjacent post named.
    pub update_account: [u8; 32],
    /// Decoded loader state, carried for archive-header binding.
    pub loader_state: LoaderStateV1,
    /// Upgrade-authority presence observed on ProgramData.
    ///
    /// Decoded evidence, not policy: the deployment-slot pin already makes any
    /// upgrade a new generation, so this is reported rather than enforced.
    pub upgrade_authority: UpgradeAuthorityV1,
    /// The parsed update.
    pub update: FullPriceUpdateV2,
    /// The archive record this witness owns.
    pub record: ArchiveRecordV2,
}

/// Refusals from the pull authentication join.
///
/// The three decoder vocabularies are carried rather than collapsed, so a
/// refusal names which capability refused. The instruction layer projects the
/// whole enum onto one stable numeric code today — the R2 plan's P0.8
/// error-granularity decision is what would widen that projection, and it is
/// still open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthV2Error {
    /// The compiled release does not match the immutable spec.
    ReleaseMismatch,
    /// The release names a loader other than the pinned Upgradeable Loader.
    UnsupportedLoader,
    /// The presented receiver program is not the pinned key.
    WrongReceiverProgram,
    /// The presented ProgramData account is not the pinned key.
    WrongProgramData,
    /// The receiver program does not link to the pinned ProgramData account.
    ProgramDataLinkMismatch,
    /// The ProgramData account records a different deployment slot.
    DeploymentSlotMismatch,
    /// The `Config` account is not the pinned key or is not receiver-owned.
    WrongConfig,
    /// A `Config` data account was presented as executable.
    ExecutableConfig,
    /// The complete `Config` body digests to a different governance
    /// generation.
    ConfigDigestMismatch,
    /// The Clock projection did not come from the canonical sysvar.
    WrongClockSysvar,
    /// The Clock account was not owned by the canonical Sysvar program.
    WrongClockSysvarOwner,
    /// The Clock is before this release's activation instant.
    ReleaseNotActive,
    /// The preceding instruction did not invoke the receiver program.
    WrongPostProgram,
    /// The post named a different `Config` account.
    WrongPostConfig,
    /// The post named a different update account than the one presented.
    WrongPostUpdate,
    /// The post's write authority is not the one the update records.
    WrongPostWriteAuthority,
    /// The update claims a receiver-write slot in the future.
    FuturePostedSlot,
    /// The update's receiver-write slot is outside the slot freshness bound.
    StalePostedSlot,
    /// The publish time is further ahead than the skew allowance.
    FuturePublishTime,
    /// The publish time is outside the seconds freshness bound.
    StalePublishTime,
    /// The named boundary has not matured past its grace delay.
    BoundaryNotMature,
    /// The widened confidence exceeds an admitted cap.
    ConfidenceCapExceeded,
    /// A freshness or confidence computation left its integer envelope.
    ArithmeticOverflow,
    /// The Clock sysvar account is malformed.
    MalformedClock,
    /// The Upgradeable Loader decoder refused.
    Loader(LoaderStateError),
    /// The Instructions-sysvar decoder refused.
    Sysvar(InstructionsSysvarError),
    /// The `PriceUpdateV2` parser refused.
    Parser(PythReceiverError),
    /// The crossing rule refused.
    Crossing(CrossingError),
}

/// The full runtime join needed before one ephemeral update may become
/// canonical archive data.
///
/// Note what is **absent** from this struct: there is no `loader_state` field
/// and no `immediate_post` field. Both are derived inside
/// [`authenticate_pull_update_v2`] from the accounts below, which is what makes
/// them unassertable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullAuthenticationV2<'a> {
    /// The compiled release triple selected for this spec.
    pub release: PullReleaseV2,
    /// The immutable spec, already decoded from its authenticated account.
    pub spec: SourceSpecV2,
    /// The receiver program account.
    pub receiver_program: LoaderAccountViewV1<'a>,
    /// The receiver's ProgramData account.
    pub receiver_programdata: LoaderAccountViewV1<'a>,
    /// The receiver `Config` account.
    pub receiver_config: AccountViewV2<'a>,
    /// The ephemeral price-update account.
    pub update: PriceUpdateAccountViewV1<'a>,
    /// The canonical Instructions sysvar account.
    pub instructions_sysvar: AccountViewV2<'a>,
    /// The Clock projection, from [`decode_clock_view`].
    pub clock: ClockViewV1,
    /// The exact bucket this append claims.
    ///
    /// Supplied by the caller only in the sense that the *archive cursor*
    /// supplies it: the instruction layer reads it out of authenticated
    /// archive state, never out of instruction data.
    pub bucket: u64,
}

/// Project the canonical Clock sysvar account, keeping the signed timestamp.
pub fn decode_clock_view(account: AccountViewV2<'_>) -> Result<ClockViewV1, AuthV2Error> {
    if account.owner != SYSVAR_OWNER_ID {
        return Err(AuthV2Error::WrongClockSysvarOwner);
    }
    if account.executable {
        return Err(AuthV2Error::MalformedClock);
    }
    if account.data.len() != CLOCK_SYSVAR_LEN {
        return Err(AuthV2Error::MalformedClock);
    }
    let mut slot = [0_u8; 8];
    slot.copy_from_slice(&account.data[..8]);
    let mut unix = [0_u8; 8];
    unix.copy_from_slice(
        &account.data[CLOCK_UNIX_TIMESTAMP_OFFSET..CLOCK_UNIX_TIMESTAMP_OFFSET + 8],
    );
    Ok(ClockViewV1 {
        key: account.key,
        slot: u64::from_le_bytes(slot),
        unix_timestamp: i64::from_le_bytes(unix),
    })
}

/// Authenticate and normalize one pull update for one exact bucket.
pub fn authenticate_pull_update_v2(
    auth: PullAuthenticationV2<'_>,
) -> Result<AuthenticatedPullUpdateV2, AuthV2Error> {
    let fields = auth.spec.fields();

    /* The compiled release and the immutable spec must name one identity. A
     * spec that names a release this ELF does not carry is not an error to be
     * adapted around: the registry is inert data and this is its match. */
    if auth.release.source_adapter_id != fields.source_adapter_id
        || auth.release.source_adapter_version != fields.source_adapter_version
        || auth.release.parser_id != fields.parser_id
        || auth.release.parser_version != fields.parser_version
        || auth.release.receiver_program != fields.receiver_program
    {
        return Err(AuthV2Error::ReleaseMismatch);
    }
    /* The loader decoder pins the Upgradeable Loader itself, so a release
     * naming any other loader could never be satisfied.  Refusing here keeps
     * the constant honest instead of silently unused. */
    if auth.release.upgradeable_loader != UPGRADEABLE_LOADER_ID {
        return Err(AuthV2Error::UnsupportedLoader);
    }

    if auth.receiver_program.key() != fields.receiver_program {
        return Err(AuthV2Error::WrongReceiverProgram);
    }
    if auth.receiver_programdata.key() != fields.receiver_programdata {
        return Err(AuthV2Error::WrongProgramData);
    }
    /* Owner, executability, variant tags, the program-to-ProgramData link, and
     * the slot all come from the pinned decoder.  Nothing here reads loader
     * bytes by hand, and the decoder's revoked-authority finding -- bytes
     * [13..45) of a `None` ProgramData still hold the previous authority -- is
     * why that matters. */
    let pair = decode_loader_pair_v1(auth.receiver_program, auth.receiver_programdata)
        .map_err(AuthV2Error::Loader)?;
    if pair.state.linked_programdata != fields.receiver_programdata {
        return Err(AuthV2Error::ProgramDataLinkMismatch);
    }
    if pair.state.deployment_slot != fields.programdata_deployment_slot {
        return Err(AuthV2Error::DeploymentSlotMismatch);
    }

    if auth.receiver_config.key != fields.receiver_config
        || auth.receiver_config.owner != fields.receiver_program
    {
        return Err(AuthV2Error::WrongConfig);
    }
    if auth.receiver_config.executable {
        return Err(AuthV2Error::ExecutableConfig);
    }
    if config_byte_digest(auth.receiver_config.data) != fields.config_digest {
        return Err(AuthV2Error::ConfigDigestMismatch);
    }

    if auth.clock.key != auth.release.clock_sysvar {
        return Err(AuthV2Error::WrongClockSysvar);
    }
    if auth.clock.unix_timestamp < auth.release.activation_unix_timestamp {
        return Err(AuthV2Error::ReleaseNotActive);
    }

    /* Adjacency is structural, not asserted: the sysvar decoder reads the
     * current index from the trailer and the post from `current - 1`, so the
     * model's `PostNotAdjacent` refusal is unreachable by construction rather
     * than checked.  A set/post/restore sandwich moves the post away from
     * `current - 1` and lands on the program/config/update refusals below. */
    let sysvar = InstructionsSysvarV1::new(
        auth.instructions_sysvar.key,
        auth.instructions_sysvar.owner,
        auth.instructions_sysvar.executable,
        auth.instructions_sysvar.data,
    )
    .map_err(AuthV2Error::Sysvar)?;
    let post = sysvar
        .immediate_post_v2(auth.release.post_abi)
        .map_err(AuthV2Error::Sysvar)?;
    if post.program != fields.receiver_program {
        return Err(AuthV2Error::WrongPostProgram);
    }
    if post.config != fields.receiver_config {
        return Err(AuthV2Error::WrongPostConfig);
    }
    if post.update_account != auth.update.key() {
        return Err(AuthV2Error::WrongPostUpdate);
    }

    let update = parse_full_price_update_v2(
        auth.update,
        fields.receiver_program,
        fields.provider_feed_id,
    )
    .map_err(AuthV2Error::Parser)?;
    if post.write_authority != update.write_authority {
        return Err(AuthV2Error::WrongPostWriteAuthority);
    }

    if update.posted_slot > auth.clock.slot {
        return Err(AuthV2Error::FuturePostedSlot);
    }
    if auth.clock.slot - update.posted_slot > fields.max_staleness_slots {
        return Err(AuthV2Error::StalePostedSlot);
    }
    let latest_publish = auth
        .clock
        .unix_timestamp
        .checked_add(
            i64::try_from(fields.max_future_seconds)
                .map_err(|_| AuthV2Error::ArithmeticOverflow)?,
        )
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    if update.publish_time > latest_publish {
        return Err(AuthV2Error::FuturePublishTime);
    }
    let oldest_publish = auth
        .clock
        .unix_timestamp
        .checked_sub(
            i64::try_from(fields.max_staleness_seconds)
                .map_err(|_| AuthV2Error::ArithmeticOverflow)?,
        )
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    if update.publish_time < oldest_publish {
        return Err(AuthV2Error::StalePublishTime);
    }

    let boundary = boundary_instant(
        fields.selection_rule,
        fields.grid_origin_unix_seconds,
        fields.bucket_seconds,
        auth.bucket,
    )
    .map_err(AuthV2Error::Crossing)?;
    let mature_at = boundary
        .checked_add(fields.boundary_grace_seconds)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    if auth.clock.unix_timestamp < mature_at {
        return Err(AuthV2Error::BoundaryNotMature);
    }

    enforce_confidence_caps(update, &fields)?;
    let record = record_from_witness(
        fields.selection_rule,
        fields.grid_origin_unix_seconds,
        fields.bucket_seconds,
        auth.bucket,
        update,
        fields.normalized_decimals,
        fields.confidence_multiplier,
    )
    .map_err(AuthV2Error::Crossing)?;

    Ok(AuthenticatedPullUpdateV2 {
        source_identity: auth.spec.feed_id(),
        update_account: auth.update.key(),
        loader_state: pair.state,
        upgrade_authority: pair.upgrade_authority,
        update,
        record,
    })
}

/// Both confidence caps, applied to the widened half-width.
///
/// The relative cap is applied to the raw widened value at source scale and the
/// absolute cap to its ceiling-normalized form: rounding a half-width *inward*
/// before a cap could let a too-wide interval pass.
fn enforce_confidence_caps(
    update: FullPriceUpdateV2,
    fields: &super::spec::SourceSpecFieldsV2,
) -> Result<(), AuthV2Error> {
    if update.price <= 0 {
        return Err(AuthV2Error::Parser(PythReceiverError::InvalidPrice));
    }
    let widened = u128::from(update.confidence)
        .checked_mul(u128::from(fields.confidence_multiplier))
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    let relative_left = widened
        .checked_mul(10_000)
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    let relative_right = u128::try_from(update.price)
        .map_err(|_| AuthV2Error::Parser(PythReceiverError::InvalidPrice))?
        .checked_mul(u128::from(fields.max_confidence_bps))
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    if relative_left > relative_right {
        return Err(AuthV2Error::ConfidenceCapExceeded);
    }
    let normalized = normalize_unsigned_ceil(widened, update.exponent, fields.normalized_decimals)
        .map_err(|_| AuthV2Error::ArithmeticOverflow)?;
    if normalized > fields.max_confidence_atoms {
        return Err(AuthV2Error::ConfidenceCapExceeded);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions_sysvar::{PostAbiPositionsV1, INSTRUCTIONS_SYSVAR_ID, SYSVAR_OWNER_ID};
    use crate::loader_state::{
        LOADER_TAG_PROGRAM, LOADER_TAG_PROGRAMDATA, OPTION_SOME, PROGRAMDATA_METADATA_LEN,
    };
    use crate::pyth_receiver::PRICE_UPDATE_V2_ACCOUNT_LEN;
    use crate::source_identity::{fixture, CLOCK_SYSVAR_ID};
    use crate::source_v2::crossing::SELECTION_CROSSING_V1;
    use crate::source_v2::spec::{
        SourceSpecFieldsV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
    };

    const UPDATE_KEY: [u8; 32] = [0xc4; 32];
    const WRITE_AUTHORITY: [u8; 32] = [0x11; 32];
    const CONFIG_BYTES: &[u8] = b"fabricated-config-generation";
    const BUCKET: u64 = 169_999_999;
    const CLOCK_SLOT: u64 = 250_000_050;
    const CLOCK_UNIX: i64 = 1_700_000_020;

    fn spec_fields() -> SourceSpecFieldsV2 {
        SourceSpecFieldsV2 {
            source_adapter_id: fixture::SOURCE_ADAPTER_ID,
            source_adapter_version: fixture::SOURCE_ADAPTER_VERSION,
            parser_id: fixture::PARSER_ID,
            parser_version: fixture::PARSER_VERSION,
            receiver_program: fixture::RECEIVER_PROGRAM,
            receiver_programdata: fixture::RECEIVER_PROGRAMDATA,
            receiver_config: fixture::RECEIVER_CONFIG,
            config_digest: config_byte_digest(CONFIG_BYTES),
            provider_feed_id: fixture::PROVIDER_FEED_ID,
            programdata_deployment_slot: fixture::PROGRAMDATA_DEPLOYMENT_SLOT,
            base_asset_id: fixture::BASE_ASSET_ID,
            quote_asset_id: fixture::QUOTE_ASSET_ID,
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 8,
            grid_family_id: 4,
            grid_version: 9,
            grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
            bucket_seconds: 10,
            boundary_grace_seconds: 10,
            max_staleness_slots: 100,
            max_staleness_seconds: 240,
            max_future_seconds: 15,
            max_confidence_atoms: 1_000_000_000_000,
            max_confidence_bps: 500,
            confidence_multiplier: 3,
            selection_rule: SELECTION_CROSSING_V1,
        }
    }

    fn spec() -> SourceSpecV2 {
        SourceSpecV2::new(spec_fields()).expect("valid fixture spec")
    }

    /// Byte image of `UpgradeableLoaderState::Program { programdata_address }`.
    fn program_body(programdata: [u8; 32]) -> Vec<u8> {
        let mut out = LOADER_TAG_PROGRAM.to_le_bytes().to_vec();
        out.extend_from_slice(&programdata);
        out
    }

    /// Byte image of `UpgradeableLoaderState::ProgramData { slot, Some(a) }`.
    fn programdata_body(slot: u64, authority: [u8; 32]) -> Vec<u8> {
        let mut out = LOADER_TAG_PROGRAMDATA.to_le_bytes().to_vec();
        out.extend_from_slice(&slot.to_le_bytes());
        out.push(OPTION_SOME);
        out.extend_from_slice(&authority);
        assert_eq!(out.len(), PROGRAMDATA_METADATA_LEN);
        out.extend_from_slice(b"fabricated-receiver-elf");
        out
    }

    fn update_body(publish: i64, prev: i64, posted_slot: u64) -> Vec<u8> {
        let mut out = vec![0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN];
        out[..8].copy_from_slice(&crate::pyth_receiver::PRICE_UPDATE_V2_DISCRIMINATOR);
        out[8..40].copy_from_slice(&WRITE_AUTHORITY);
        out[40] = crate::pyth_receiver::VERIFICATION_LEVEL_FULL;
        out[41..73].copy_from_slice(&fixture::PROVIDER_FEED_ID);
        out[73..81].copy_from_slice(&123_456_789_i64.to_le_bytes());
        out[81..89].copy_from_slice(&12_345_u64.to_le_bytes());
        out[89..93].copy_from_slice(&(-8_i32).to_le_bytes());
        out[93..101].copy_from_slice(&publish.to_le_bytes());
        out[101..109].copy_from_slice(&prev.to_le_bytes());
        out[109..117].copy_from_slice(&123_450_000_i64.to_le_bytes());
        out[117..125].copy_from_slice(&20_000_u64.to_le_bytes());
        out[125..133].copy_from_slice(&posted_slot.to_le_bytes());
        out
    }

    /// Serialize an Instructions-sysvar image with one post at index 0 and the
    /// consuming instruction at index 1.
    fn sysvar_body(
        post_program: [u8; 32],
        metas: &[[u8; 32]],
        current: u16,
        post_index_present: bool,
    ) -> Vec<u8> {
        let flagged: Vec<(u8, [u8; 32])> = metas
            .iter()
            .copied()
            .enumerate()
            .map(|(position, address)| (fixture::POST_ABI.account_flags[position], address))
            .collect();
        sysvar_body_exact(
            post_program,
            &flagged,
            &fixture::POST_UPDATE_DISCRIMINATOR,
            current,
            post_index_present,
        )
    }

    fn sysvar_body_exact(
        post_program: [u8; 32],
        metas: &[(u8, [u8; 32])],
        post_data: &[u8],
        current: u16,
        post_index_present: bool,
    ) -> Vec<u8> {
        let mut instructions: Vec<(Vec<u8>, [u8; 32])> = Vec::new();
        if post_index_present {
            let mut body = Vec::new();
            body.extend_from_slice(&(metas.len() as u16).to_le_bytes());
            for (flags, meta) in metas {
                body.push(*flags);
                body.extend_from_slice(meta);
            }
            body.extend_from_slice(&post_program);
            body.extend_from_slice(&u16::try_from(post_data.len()).unwrap().to_le_bytes());
            body.extend_from_slice(post_data);
            instructions.push((body, post_program));
        }
        // The consuming instruction: no accounts, no data.
        let mut consuming = Vec::new();
        consuming.extend_from_slice(&0_u16.to_le_bytes());
        consuming.extend_from_slice(&[0x99; 32]);
        consuming.extend_from_slice(&0_u16.to_le_bytes());
        instructions.push((consuming, [0x99; 32]));

        let count = instructions.len() as u16;
        let table_bytes = 2 + 2 * instructions.len();
        let mut offsets = Vec::new();
        let mut bodies = Vec::new();
        let mut at = table_bytes;
        for (body, _) in &instructions {
            offsets.push(at as u16);
            at += body.len();
            bodies.extend_from_slice(body);
        }
        let mut out = count.to_le_bytes().to_vec();
        for offset in offsets {
            out.extend_from_slice(&offset.to_le_bytes());
        }
        out.extend_from_slice(&bodies);
        out.extend_from_slice(&current.to_le_bytes());
        out
    }

    /// The seven-meta post layout the fixture ABI addresses: payer, VAA,
    /// config, treasury, update, system, write authority.
    fn post_metas(config: [u8; 32], update: [u8; 32], authority: [u8; 32]) -> Vec<[u8; 32]> {
        vec![
            [0x01; 32], [0x02; 32], config, [0x04; 32], update, [0x06; 32], authority,
        ]
    }

    struct Bytes {
        program: Vec<u8>,
        programdata: Vec<u8>,
        config: Vec<u8>,
        update: Vec<u8>,
        sysvar: Vec<u8>,
    }

    fn bytes() -> Bytes {
        Bytes {
            program: program_body(fixture::RECEIVER_PROGRAMDATA),
            programdata: programdata_body(fixture::PROGRAMDATA_DEPLOYMENT_SLOT, [0x77; 32]),
            config: CONFIG_BYTES.to_vec(),
            update: update_body(1_700_000_011, 1_699_999_999, 250_000_000),
            sysvar: sysvar_body(
                fixture::RECEIVER_PROGRAM,
                &post_metas(fixture::RECEIVER_CONFIG, UPDATE_KEY, WRITE_AUTHORITY),
                1,
                true,
            ),
        }
    }

    fn auth<'a>(b: &'a Bytes) -> PullAuthenticationV2<'a> {
        PullAuthenticationV2 {
            release: fixture::RELEASE,
            spec: spec(),
            receiver_program: LoaderAccountViewV1::new(
                fixture::RECEIVER_PROGRAM,
                UPGRADEABLE_LOADER_ID,
                true,
                &b.program,
            ),
            receiver_programdata: LoaderAccountViewV1::new(
                fixture::RECEIVER_PROGRAMDATA,
                UPGRADEABLE_LOADER_ID,
                false,
                &b.programdata,
            ),
            receiver_config: AccountViewV2::new(
                fixture::RECEIVER_CONFIG,
                fixture::RECEIVER_PROGRAM,
                false,
                &b.config,
            ),
            update: PriceUpdateAccountViewV1::new(
                UPDATE_KEY,
                fixture::RECEIVER_PROGRAM,
                false,
                &b.update,
            ),
            instructions_sysvar: AccountViewV2::new(
                INSTRUCTIONS_SYSVAR_ID,
                SYSVAR_OWNER_ID,
                false,
                &b.sysvar,
            ),
            clock: ClockViewV1 {
                key: CLOCK_SYSVAR_ID,
                slot: CLOCK_SLOT,
                unix_timestamp: CLOCK_UNIX,
            },
            bucket: BUCKET,
        }
    }

    #[test]
    fn the_full_join_admits_a_well_formed_fixture() {
        let b = bytes();
        let admitted = authenticate_pull_update_v2(auth(&b)).expect("fixture authenticates");
        assert_eq!(admitted.source_identity, spec().feed_id());
        assert_ne!(admitted.source_identity, UPDATE_KEY);
        assert_eq!(admitted.update_account, UPDATE_KEY);
        assert_eq!(
            admitted.loader_state.linked_programdata,
            fixture::RECEIVER_PROGRAMDATA
        );
        assert_eq!(
            admitted.loader_state.deployment_slot,
            fixture::PROGRAMDATA_DEPLOYMENT_SLOT
        );
        assert_eq!(admitted.record.bucket, BUCKET);
        assert_eq!(admitted.record.sequence, 1_700_000_011);
        assert_eq!(admitted.record.publish_slot, 250_000_000);
    }

    #[test]
    fn the_canonical_identity_is_the_spec_not_the_ephemeral_account() {
        // The same spec, posted through a different ephemeral account, is the
        // same source identity.  That is the whole point of v2.
        let mut b = bytes();
        let other_key = [0xab; 32];
        b.sysvar = sysvar_body(
            fixture::RECEIVER_PROGRAM,
            &post_metas(fixture::RECEIVER_CONFIG, other_key, WRITE_AUTHORITY),
            1,
            true,
        );
        let mut case = auth(&b);
        case.update =
            PriceUpdateAccountViewV1::new(other_key, fixture::RECEIVER_PROGRAM, false, &b.update);
        let admitted = authenticate_pull_update_v2(case).expect("other ephemeral key admits");
        assert_eq!(admitted.source_identity, spec().feed_id());
        assert_eq!(admitted.update_account, other_key);
    }

    #[test]
    fn deployment_substitution_and_slot_drift_refuse() {
        let b = bytes();
        let mut case = auth(&b);
        case.receiver_program =
            LoaderAccountViewV1::new([0xf0; 32], UPGRADEABLE_LOADER_ID, true, &b.program);
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongReceiverProgram)
        );

        case = auth(&b);
        case.receiver_programdata =
            LoaderAccountViewV1::new([0xf1; 32], UPGRADEABLE_LOADER_ID, false, &b.programdata);
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongProgramData)
        );

        // A receiver program whose body links elsewhere: the decoder catches
        // the mismatch against the presented account before this module's own
        // link comparison.
        let elsewhere = program_body([0xf2; 32]);
        case = auth(&b);
        case.receiver_program = LoaderAccountViewV1::new(
            fixture::RECEIVER_PROGRAM,
            UPGRADEABLE_LOADER_ID,
            true,
            &elsewhere,
        );
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::Loader(
                LoaderStateError::ProgramDataLinkMismatch
            ))
        );

        // An in-place upgrade rewrites ProgramData with a new slot.  That is a
        // different source generation, and it fails closed.
        let upgraded = programdata_body(fixture::PROGRAMDATA_DEPLOYMENT_SLOT + 1, [0x77; 32]);
        case = auth(&b);
        case.receiver_programdata = LoaderAccountViewV1::new(
            fixture::RECEIVER_PROGRAMDATA,
            UPGRADEABLE_LOADER_ID,
            false,
            &upgraded,
        );
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::DeploymentSlotMismatch)
        );
    }

    #[test]
    fn a_non_loader_owner_or_non_executable_receiver_refuses() {
        let b = bytes();
        let mut case = auth(&b);
        case.receiver_program =
            LoaderAccountViewV1::new(fixture::RECEIVER_PROGRAM, [0xee; 32], true, &b.program);
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::Loader(LoaderStateError::WrongLoaderOwner))
        );

        case = auth(&b);
        case.receiver_program = LoaderAccountViewV1::new(
            fixture::RECEIVER_PROGRAM,
            UPGRADEABLE_LOADER_ID,
            false,
            &b.program,
        );
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::Loader(LoaderStateError::ProgramNotExecutable))
        );
    }

    #[test]
    fn every_config_byte_is_the_governance_generation() {
        let b = bytes();
        for at in 0..b.config.len() {
            let mut hostile = b.config.clone();
            hostile[at] ^= 1;
            let mut case = auth(&b);
            case.receiver_config = AccountViewV2::new(
                fixture::RECEIVER_CONFIG,
                fixture::RECEIVER_PROGRAM,
                false,
                &hostile,
            );
            assert_eq!(
                authenticate_pull_update_v2(case),
                Err(AuthV2Error::ConfigDigestMismatch)
            );
        }

        let mut case = auth(&b);
        case.receiver_config =
            AccountViewV2::new([0xf4; 32], fixture::RECEIVER_PROGRAM, false, &b.config);
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongConfig)
        );

        case = auth(&b);
        case.receiver_config =
            AccountViewV2::new(fixture::RECEIVER_CONFIG, [0xf5; 32], false, &b.config);
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongConfig)
        );

        case = auth(&b);
        case.receiver_config = AccountViewV2::new(
            fixture::RECEIVER_CONFIG,
            fixture::RECEIVER_PROGRAM,
            true,
            &b.config,
        );
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ExecutableConfig)
        );
    }

    #[test]
    fn the_post_join_cannot_be_asserted_only_read() {
        let b = bytes();

        // No preceding instruction at all: the consuming instruction is first.
        let alone = Bytes {
            sysvar: sysvar_body(fixture::RECEIVER_PROGRAM, &[], 0, false),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            update: b.update.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&alone)),
            Err(AuthV2Error::Sysvar(
                InstructionsSysvarError::NoPrecedingInstruction
            ))
        );

        // The post is present but not adjacent: a set/post/restore sandwich
        // leaves some other instruction at `current - 1`.
        let restored = Bytes {
            sysvar: sysvar_body(
                [0xac; 32],
                &post_metas(fixture::RECEIVER_CONFIG, UPDATE_KEY, WRITE_AUTHORITY),
                1,
                true,
            ),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            update: b.update.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&restored)),
            Err(AuthV2Error::WrongPostProgram)
        );

        for (metas, expected) in [
            (
                post_metas([0xab; 32], UPDATE_KEY, WRITE_AUTHORITY),
                AuthV2Error::WrongPostConfig,
            ),
            (
                post_metas(fixture::RECEIVER_CONFIG, [0xaa; 32], WRITE_AUTHORITY),
                AuthV2Error::WrongPostUpdate,
            ),
            (
                post_metas(fixture::RECEIVER_CONFIG, UPDATE_KEY, [0xad; 32]),
                AuthV2Error::WrongPostWriteAuthority,
            ),
        ] {
            let case = Bytes {
                sysvar: sysvar_body(fixture::RECEIVER_PROGRAM, &metas, 1, true),
                program: b.program.clone(),
                programdata: b.programdata.clone(),
                config: b.config.clone(),
                update: b.update.clone(),
            };
            assert_eq!(authenticate_pull_update_v2(auth(&case)), Err(expected));
        }
    }

    #[test]
    fn a_post_too_short_for_the_abi_refuses_rather_than_reads_past_it() {
        let b = bytes();
        let short = Bytes {
            sysvar: sysvar_body(
                fixture::RECEIVER_PROGRAM,
                &[[0x01; 32], [0x02; 32], fixture::RECEIVER_CONFIG],
                1,
                true,
            ),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            update: b.update.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&short)),
            Err(AuthV2Error::Sysvar(
                InstructionsSysvarError::WrongPostAccountCount
            ))
        );
    }

    #[test]
    fn the_post_discriminator_count_and_effective_flags_are_load_bearing() {
        let b = bytes();
        let addresses = post_metas(fixture::RECEIVER_CONFIG, UPDATE_KEY, WRITE_AUTHORITY);
        let canonical: Vec<(u8, [u8; 32])> = addresses
            .iter()
            .copied()
            .enumerate()
            .map(|(position, address)| (fixture::POST_ABI.account_flags[position], address))
            .collect();

        let mut discriminator = fixture::POST_UPDATE_DISCRIMINATOR;
        discriminator[0] ^= 1;
        let wrong_discriminator = Bytes {
            sysvar: sysvar_body_exact(
                fixture::RECEIVER_PROGRAM,
                &canonical,
                &discriminator,
                1,
                true,
            ),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            update: b.update.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&wrong_discriminator)),
            Err(AuthV2Error::Sysvar(
                InstructionsSysvarError::WrongPostDiscriminator
            ))
        );

        let extra = canonical
            .iter()
            .copied()
            .chain(core::iter::once((0, [0x88; 32])))
            .collect::<Vec<_>>();
        let wrong_count = Bytes {
            sysvar: sysvar_body_exact(
                fixture::RECEIVER_PROGRAM,
                &extra,
                &fixture::POST_UPDATE_DISCRIMINATOR,
                1,
                true,
            ),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            update: b.update.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&wrong_count)),
            Err(AuthV2Error::Sysvar(
                InstructionsSysvarError::WrongPostAccountCount
            ))
        );

        let mut wrong_flags = canonical;
        wrong_flags[2].0 ^= crate::instructions_sysvar::META_FLAG_IS_WRITABLE;
        let wrong_flags = Bytes {
            sysvar: sysvar_body_exact(
                fixture::RECEIVER_PROGRAM,
                &wrong_flags,
                &fixture::POST_UPDATE_DISCRIMINATOR,
                1,
                true,
            ),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            update: b.update.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&wrong_flags)),
            Err(AuthV2Error::Sysvar(
                InstructionsSysvarError::WrongPostAccountFlags
            ))
        );
    }

    #[test]
    fn an_aliased_post_abi_refuses_before_it_can_confuse_two_roles() {
        let b = bytes();
        let mut case = auth(&b);
        case.release.post_abi.positions = PostAbiPositionsV1 {
            config: 2,
            update_account: 2,
            write_authority: 6,
        };
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::Sysvar(
                InstructionsSysvarError::AliasedPostAbiPositions
            ))
        );
    }

    #[test]
    fn clock_identity_activation_and_both_freshness_envelopes_fail_closed() {
        let b = bytes();
        let mut case = auth(&b);
        case.clock.key = [0xf2; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongClockSysvar)
        );

        case = auth(&b);
        case.release.activation_unix_timestamp = CLOCK_UNIX + 1;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ReleaseNotActive)
        );

        case = auth(&b);
        case.clock.slot = 249_999_999;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::FuturePostedSlot)
        );

        case = auth(&b);
        case.clock.slot = 250_000_101;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::StalePostedSlot)
        );

        case = auth(&b);
        case.clock.unix_timestamp = 1_700_000_009;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::BoundaryNotMature)
        );

        let future = Bytes {
            update: update_body(1_700_000_036, 1_699_999_999, 250_000_000),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            sysvar: b.sysvar.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&future)),
            Err(AuthV2Error::FuturePublishTime)
        );

        let stale = Bytes {
            update: update_body(1_699_999_779, 1_699_999_778, 250_000_000),
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            sysvar: b.sysvar.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&stale)),
            Err(AuthV2Error::StalePublishTime)
        );
    }

    #[test]
    fn the_boundary_grace_is_a_clock_gate_not_a_finality_claim() {
        // T(k) = 1_700_000_000, grace 10, so the first admissible Clock is
        // 1_700_000_010 -- one second earlier refuses, and the refusal is
        // about the local Clock only.
        let b = bytes();
        let mut case = auth(&b);
        case.clock.unix_timestamp = 1_700_000_010;
        assert!(authenticate_pull_update_v2(case).is_ok());
        case = auth(&b);
        case.clock.unix_timestamp = 1_700_000_009;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::BoundaryNotMature)
        );
    }

    #[test]
    fn a_release_this_elf_does_not_carry_refuses() {
        let b = bytes();
        for mutate in [
            (|r: &mut PullReleaseV2| r.parser_version += 1) as fn(&mut PullReleaseV2),
            |r| r.parser_id += 1,
            |r| r.source_adapter_version += 1,
            |r| r.source_adapter_id[0] ^= 1,
            |r| r.receiver_program[0] ^= 1,
        ] {
            let mut case = auth(&b);
            mutate(&mut case.release);
            assert_eq!(
                authenticate_pull_update_v2(case),
                Err(AuthV2Error::ReleaseMismatch)
            );
        }

        let mut case = auth(&b);
        case.release.upgradeable_loader = [0x01; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::UnsupportedLoader)
        );
    }

    #[test]
    fn the_wrong_feed_and_the_wrong_update_owner_refuse_in_the_parser() {
        let b = bytes();
        let mut case = auth(&b);
        case.update = PriceUpdateAccountViewV1::new(UPDATE_KEY, [0xf3; 32], false, &b.update);
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::Parser(PythReceiverError::WrongOwner))
        );

        let mut wrong_feed = b.update.clone();
        wrong_feed[41] ^= 1;
        let hostile = Bytes {
            update: wrong_feed,
            program: b.program.clone(),
            programdata: b.programdata.clone(),
            config: b.config.clone(),
            sysvar: b.sysvar.clone(),
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&hostile)),
            Err(AuthV2Error::Parser(PythReceiverError::WrongFeed))
        );
    }

    #[test]
    fn confidence_caps_are_applied_to_the_widened_half_width() {
        let b = bytes();
        let mut case = auth(&b);
        let mut fields = spec_fields();
        fields.max_confidence_bps = 1;
        case.spec = SourceSpecV2::new(fields).unwrap();
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ConfidenceCapExceeded)
        );

        case = auth(&b);
        fields = spec_fields();
        fields.max_confidence_atoms = 1;
        case.spec = SourceSpecV2::new(fields).unwrap();
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ConfidenceCapExceeded)
        );
    }

    #[test]
    fn a_non_witnessing_update_refuses_at_the_crossing_rule() {
        // T(BUCKET) = 1_700_000_000, and this update's window closes before it.
        let b = Bytes {
            update: update_body(1_699_999_999, 1_699_999_990, 250_000_000),
            program: bytes().program,
            programdata: bytes().programdata,
            config: bytes().config,
            sysvar: bytes().sysvar,
        };
        assert_eq!(
            authenticate_pull_update_v2(auth(&b)),
            Err(AuthV2Error::Crossing(CrossingError::NotBoundaryWitness))
        );
    }

    #[test]
    fn the_clock_projection_keeps_the_sign_and_refuses_malformed_bodies() {
        let mut body = vec![0_u8; CLOCK_SYSVAR_LEN];
        body[..8].copy_from_slice(&99_u64.to_le_bytes());
        body[CLOCK_UNIX_TIMESTAMP_OFFSET..].copy_from_slice(&(-5_i64).to_le_bytes());
        let view = decode_clock_view(AccountViewV2::new(
            CLOCK_SYSVAR_ID,
            SYSVAR_OWNER_ID,
            false,
            &body,
        ))
        .expect("well-formed clock");
        assert_eq!(view.slot, 99);
        assert_eq!(view.unix_timestamp, -5);

        assert_eq!(
            decode_clock_view(AccountViewV2::new(CLOCK_SYSVAR_ID, [0; 32], false, &body)),
            Err(AuthV2Error::WrongClockSysvarOwner)
        );
        assert_eq!(
            decode_clock_view(AccountViewV2::new(
                CLOCK_SYSVAR_ID,
                SYSVAR_OWNER_ID,
                true,
                &body
            )),
            Err(AuthV2Error::MalformedClock)
        );
        assert_eq!(
            decode_clock_view(AccountViewV2::new(
                CLOCK_SYSVAR_ID,
                SYSVAR_OWNER_ID,
                false,
                &body[..CLOCK_SYSVAR_LEN - 1]
            )),
            Err(AuthV2Error::MalformedClock)
        );
        let mut long = body.clone();
        long.push(0);
        assert_eq!(
            decode_clock_view(AccountViewV2::new(
                CLOCK_SYSVAR_ID,
                SYSVAR_OWNER_ID,
                false,
                &long
            )),
            Err(AuthV2Error::MalformedClock)
        );
    }
}
