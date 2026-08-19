//! Pull-profile identity and atomic-post authentication model.
//!
//! This is an executable adapter contract, not a production registry entry.
//! The future SBF seam must obtain [`LoaderStateV1`] from the pinned official
//! Upgradeable Loader decoder and [`ImmediatePostV1`] from the Instructions
//! sysvar. Callers must never be allowed to assert either projection in
//! instruction data. No post-cutover identity is compiled here, so the default
//! runtime registry remains empty and continues to refuse `0x79`.

use sha2::{Digest, Sha256};

use crate::crossing_v1::{boundary_instant, record_from_witness, ArchiveRecordV2, CrossingError};
use crate::spec_v2::SourceSpecV2;
use crate::{parse_full_price_update_v2, AccountView, Error, FullPriceUpdateV2};

/// Metadata-bearing runtime account view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAccountView<'a> {
    pub key: [u8; 32],
    pub owner: [u8; 32],
    pub executable: bool,
    pub data: &'a [u8],
}

/// Output of the future pinned Upgradeable Loader parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoaderStateV1 {
    /// ProgramData address encoded by the receiver program account.
    pub linked_programdata: [u8; 32],
    /// Deployment slot encoded by the linked ProgramData account.
    pub deployment_slot: u64,
}

/// Projection of the immediately preceding receiver-post instruction.
///
/// The future SBF adapter must derive this from the canonical Instructions
/// sysvar and exact reviewed post-instruction ABI. This model checks the join;
/// it does not pretend a caller-provided struct proves instruction history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmediatePostV1 {
    pub instruction_index: u16,
    pub consuming_instruction_index: u16,
    pub program: [u8; 32],
    pub config: [u8; 32],
    pub update_account: [u8; 32],
    pub write_authority: [u8; 32],
}

/// Clock observation loaded from the exact canonical sysvar account.
///
/// There is deliberately no `finalized` bit: RPC commitment is a client/
/// ledger property and cannot be proven by an executing instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockViewV1 {
    pub key: [u8; 32],
    pub slot: u64,
    pub unix_timestamp: i64,
}

/// Compile-time release identity. Production keeps this registry empty until
/// post-cutover bytes and the exact loader/post parsers are frozen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullReleaseV2 {
    pub source_adapter_id: [u8; 32],
    pub source_adapter_version: u32,
    pub parser_id: u16,
    pub parser_version: u16,
    pub receiver_program: [u8; 32],
    pub upgradeable_loader: [u8; 32],
    pub clock_sysvar: [u8; 32],
    /// Earliest Clock time at which this identity generation may be consumed.
    pub activation_unix_timestamp: i64,
}

/// Full runtime join needed before one ephemeral update can become canonical
/// Dragon's Clutch archive data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullAuthenticationV2<'a> {
    pub release: PullReleaseV2,
    pub spec: SourceSpecV2,
    pub receiver_program: RuntimeAccountView<'a>,
    pub receiver_programdata: RuntimeAccountView<'a>,
    pub loader_state: LoaderStateV1,
    pub receiver_config: RuntimeAccountView<'a>,
    pub update: AccountView<'a>,
    pub immediate_post: ImmediatePostV1,
    pub clock: ClockViewV1,
    pub bucket: u64,
}

/// Capability emitted only after every modeled identity, time, parser, and
/// selection check succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPullUpdateV2 {
    /// Canonical immutable SourceSpec-v2 identity; never the ephemeral account.
    pub source_identity: [u8; 32],
    /// Exact ephemeral account consumed by the adjacent post join.
    pub update_account: [u8; 32],
    pub update: FullPriceUpdateV2,
    pub record: ArchiveRecordV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthV2Error {
    ReleaseMismatch,
    WrongReceiverProgram,
    WrongLoader,
    ReceiverNotExecutable,
    WrongProgramData,
    ExecutableProgramData,
    ProgramDataLinkMismatch,
    DeploymentSlotMismatch,
    WrongConfig,
    ExecutableConfig,
    ConfigDigestMismatch,
    WrongClockSysvar,
    ReleaseNotActive,
    PostNotAdjacent,
    WrongPostProgram,
    WrongPostConfig,
    WrongPostUpdate,
    WrongPostWriteAuthority,
    FuturePostedSlot,
    StalePostedSlot,
    FuturePublishTime,
    StalePublishTime,
    BoundaryNotMature,
    ConfidenceCapExceeded,
    ArithmeticOverflow,
    Parser(Error),
    Crossing(CrossingError),
}

/// SHA-256 of the complete Config account bytes. Any byte change is a new
/// immutable source generation; no field-level governance exception exists.
pub fn config_byte_digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Authenticate and normalize one pull update for the exact next bucket.
pub fn authenticate_pull_update_v2(
    auth: PullAuthenticationV2<'_>,
) -> Result<AuthenticatedPullUpdateV2, AuthV2Error> {
    let fields = auth.spec.fields();
    if auth.release.source_adapter_id != fields.source_adapter_id
        || auth.release.source_adapter_version != fields.source_adapter_version
        || auth.release.parser_id != fields.parser_id
        || auth.release.parser_version != fields.parser_version
        || auth.release.receiver_program != fields.receiver_program
    {
        return Err(AuthV2Error::ReleaseMismatch);
    }
    if auth.receiver_program.key != fields.receiver_program {
        return Err(AuthV2Error::WrongReceiverProgram);
    }
    if auth.receiver_program.owner != auth.release.upgradeable_loader {
        return Err(AuthV2Error::WrongLoader);
    }
    if !auth.receiver_program.executable {
        return Err(AuthV2Error::ReceiverNotExecutable);
    }
    if auth.receiver_programdata.key != fields.receiver_programdata
        || auth.receiver_programdata.owner != auth.release.upgradeable_loader
    {
        return Err(AuthV2Error::WrongProgramData);
    }
    if auth.receiver_programdata.executable {
        return Err(AuthV2Error::ExecutableProgramData);
    }
    if auth.loader_state.linked_programdata != fields.receiver_programdata {
        return Err(AuthV2Error::ProgramDataLinkMismatch);
    }
    if auth.loader_state.deployment_slot != fields.programdata_deployment_slot {
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
    if auth.immediate_post.instruction_index.checked_add(1)
        != Some(auth.immediate_post.consuming_instruction_index)
    {
        return Err(AuthV2Error::PostNotAdjacent);
    }
    if auth.immediate_post.program != fields.receiver_program {
        return Err(AuthV2Error::WrongPostProgram);
    }
    if auth.immediate_post.config != fields.receiver_config {
        return Err(AuthV2Error::WrongPostConfig);
    }
    if auth.immediate_post.update_account != auth.update.key {
        return Err(AuthV2Error::WrongPostUpdate);
    }

    let update = parse_full_price_update_v2(
        auth.update,
        fields.receiver_program,
        fields.provider_feed_id,
    )
    .map_err(AuthV2Error::Parser)?;
    if auth.immediate_post.write_authority != update.write_authority {
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

    enforce_confidence_caps(update, fields)?;
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
        update_account: auth.update.key,
        update,
        record,
    })
}

fn enforce_confidence_caps(
    update: FullPriceUpdateV2,
    fields: crate::spec_v2::SourceSpecFieldsV2,
) -> Result<(), AuthV2Error> {
    if update.price <= 0 {
        return Err(AuthV2Error::Parser(Error::InvalidPrice));
    }
    let widened = u128::from(update.confidence)
        .checked_mul(u128::from(fields.confidence_multiplier))
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    let relative_left = widened
        .checked_mul(10_000)
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    let relative_right = u128::try_from(update.price)
        .map_err(|_| AuthV2Error::Parser(Error::InvalidPrice))?
        .checked_mul(u128::from(fields.max_confidence_bps))
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    if relative_left > relative_right {
        return Err(AuthV2Error::ConfidenceCapExceeded);
    }
    let normalized = normalize_unsigned_ceil(widened, update.exponent, fields.normalized_decimals)?;
    if normalized > fields.max_confidence_atoms {
        return Err(AuthV2Error::ConfidenceCapExceeded);
    }
    Ok(())
}

fn normalize_unsigned_ceil(
    value: u128,
    exponent: i32,
    target_decimals: u8,
) -> Result<u128, AuthV2Error> {
    let shift = exponent
        .checked_add(i32::from(target_decimals))
        .ok_or(AuthV2Error::ArithmeticOverflow)?;
    if shift >= 0 {
        let factor = pow10(u32::try_from(shift).map_err(|_| AuthV2Error::ArithmeticOverflow)?)?;
        value
            .checked_mul(factor)
            .ok_or(AuthV2Error::ArithmeticOverflow)
    } else {
        let magnitude = shift.checked_neg().ok_or(AuthV2Error::ArithmeticOverflow)?;
        let divisor =
            pow10(u32::try_from(magnitude).map_err(|_| AuthV2Error::ArithmeticOverflow)?)?;
        value
            .checked_add(divisor - 1)
            .ok_or(AuthV2Error::ArithmeticOverflow)
            .map(|rounded| rounded / divisor)
    }
}

fn pow10(exponent: u32) -> Result<u128, AuthV2Error> {
    let mut value = 1_u128;
    let mut index = 0;
    while index < exponent {
        value = value
            .checked_mul(10)
            .ok_or(AuthV2Error::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crossing_v1::SELECTION_CROSSING_V1;
    use crate::spec_v2::{
        SourceSpecFieldsV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
    };
    use crate::PRICE_UPDATE_V2_ACCOUNT_LEN;

    const ADAPTER: [u8; 32] = [0xa1; 32];
    const RECEIVER: [u8; 32] = [0xb2; 32];
    const PROGRAMDATA: [u8; 32] = [0xb3; 32];
    const CONFIG: [u8; 32] = [0xc3; 32];
    const UPDATE: [u8; 32] = [0xc4; 32];
    const FEED: [u8; 32] = [0x22; 32];
    const LOADER: [u8; 32] = [0xd5; 32];
    const CLOCK: [u8; 32] = [0xe6; 32];
    const CONFIG_BYTES: &[u8] = b"post-cutover-config-generation";
    const FIXTURE_HEX: &str = include_str!("../fixtures/price-update-v2-full.hex");

    fn fixture() -> [u8; PRICE_UPDATE_V2_ACCOUNT_LEN] {
        let source = FIXTURE_HEX.trim().as_bytes();
        let mut out = [0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN];
        let mut index = 0;
        while index < out.len() {
            out[index] = (nibble(source[index * 2]) << 4) | nibble(source[index * 2 + 1]);
            index += 1;
        }
        out
    }

    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("fixture is lowercase hexadecimal"),
        }
    }

    fn set_i64(bytes: &mut [u8; PRICE_UPDATE_V2_ACCOUNT_LEN], at: usize, value: i64) {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn spec() -> SourceSpecV2 {
        SourceSpecV2::new(SourceSpecFieldsV2 {
            source_adapter_id: ADAPTER,
            source_adapter_version: 3,
            parser_id: 6,
            parser_version: 5,
            receiver_program: RECEIVER,
            receiver_programdata: PROGRAMDATA,
            receiver_config: CONFIG,
            config_digest: config_byte_digest(CONFIG_BYTES),
            provider_feed_id: FEED,
            programdata_deployment_slot: 77,
            base_asset_id: [0x11; 32],
            quote_asset_id: [0x33; 32],
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
        })
        .unwrap()
    }

    fn release() -> PullReleaseV2 {
        PullReleaseV2 {
            source_adapter_id: ADAPTER,
            source_adapter_version: 3,
            parser_id: 6,
            parser_version: 5,
            receiver_program: RECEIVER,
            upgradeable_loader: LOADER,
            clock_sysvar: CLOCK,
            activation_unix_timestamp: 1_700_000_000,
        }
    }

    fn auth<'a>(update_bytes: &'a [u8]) -> PullAuthenticationV2<'a> {
        PullAuthenticationV2 {
            release: release(),
            spec: spec(),
            receiver_program: RuntimeAccountView {
                key: RECEIVER,
                owner: LOADER,
                executable: true,
                data: &[],
            },
            receiver_programdata: RuntimeAccountView {
                key: PROGRAMDATA,
                owner: LOADER,
                executable: false,
                data: &[],
            },
            loader_state: LoaderStateV1 {
                linked_programdata: PROGRAMDATA,
                deployment_slot: 77,
            },
            receiver_config: RuntimeAccountView {
                key: CONFIG,
                owner: RECEIVER,
                executable: false,
                data: CONFIG_BYTES,
            },
            update: AccountView {
                key: UPDATE,
                owner: RECEIVER,
                executable: false,
                data: update_bytes,
            },
            immediate_post: ImmediatePostV1 {
                instruction_index: 4,
                consuming_instruction_index: 5,
                program: RECEIVER,
                config: CONFIG,
                update_account: UPDATE,
                write_authority: [0x11; 32],
            },
            clock: ClockViewV1 {
                key: CLOCK,
                slot: 250_000_050,
                unix_timestamp: 1_700_000_020,
            },
            // T(k) = (k+1)*10 = 1_700_000_000.
            bucket: 169_999_999,
        }
    }

    #[test]
    fn ephemeral_update_joins_to_canonical_source_identity() {
        let bytes = fixture();
        let receipt = authenticate_pull_update_v2(auth(&bytes)).unwrap();
        assert_eq!(receipt.source_identity, spec().feed_id());
        assert_ne!(receipt.source_identity, UPDATE);
        assert_eq!(receipt.update_account, UPDATE);
        assert_eq!(receipt.record.bucket, 169_999_999);
        assert_eq!(receipt.record.sequence, 1_700_000_011);
    }

    #[test]
    fn programdata_config_and_release_substitutions_refuse() {
        let bytes = fixture();
        let mut case = auth(&bytes);
        case.receiver_programdata.key = [0xf0; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongProgramData)
        );
        case = auth(&bytes);
        case.loader_state.linked_programdata = [0xf1; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ProgramDataLinkMismatch)
        );
        case = auth(&bytes);
        case.loader_state.deployment_slot = 78;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::DeploymentSlotMismatch)
        );
        case = auth(&bytes);
        case.receiver_config.data = b"mutated-config";
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ConfigDigestMismatch)
        );
        case = auth(&bytes);
        case.release.parser_version += 1;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ReleaseMismatch)
        );
    }

    #[test]
    fn post_adjacency_prevents_restore_and_account_substitution() {
        let bytes = fixture();
        let mut restored = auth(&bytes);
        // A set/post/restore sequence makes post non-adjacent to consumption.
        restored.immediate_post.consuming_instruction_index = 6;
        assert_eq!(
            authenticate_pull_update_v2(restored),
            Err(AuthV2Error::PostNotAdjacent)
        );

        let mut substituted = auth(&bytes);
        substituted.update.key = [0xaa; 32];
        assert_eq!(
            authenticate_pull_update_v2(substituted),
            Err(AuthV2Error::WrongPostUpdate)
        );
        let mut wrong_config = auth(&bytes);
        wrong_config.immediate_post.config = [0xab; 32];
        assert_eq!(
            authenticate_pull_update_v2(wrong_config),
            Err(AuthV2Error::WrongPostConfig)
        );

        let mut wrong_program = auth(&bytes);
        wrong_program.immediate_post.program = [0xac; 32];
        assert_eq!(
            authenticate_pull_update_v2(wrong_program),
            Err(AuthV2Error::WrongPostProgram)
        );
        let mut wrong_authority = auth(&bytes);
        wrong_authority.immediate_post.write_authority = [0xad; 32];
        assert_eq!(
            authenticate_pull_update_v2(wrong_authority),
            Err(AuthV2Error::WrongPostWriteAuthority)
        );
    }

    #[test]
    fn clock_cutover_freshness_and_boundary_grace_fail_closed() {
        let bytes = fixture();
        let mut case = auth(&bytes);
        case.clock.key = [0xf2; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongClockSysvar)
        );
        case = auth(&bytes);
        case.release.activation_unix_timestamp = 1_700_000_021;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::ReleaseNotActive)
        );
        case = auth(&bytes);
        case.clock.slot = 249_999_999;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::FuturePostedSlot)
        );
        case = auth(&bytes);
        case.clock.slot = 250_000_101;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::StalePostedSlot)
        );
        case = auth(&bytes);
        case.clock.unix_timestamp = 1_700_000_009;
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::BoundaryNotMature)
        );

        // Time refusals are evaluated from canonical Clock and parsed update
        // bytes; no caller-provided freshness claim can override them.
        let mut future = bytes;
        set_i64(&mut future, 93, 1_700_000_036);
        assert_eq!(
            authenticate_pull_update_v2(auth(&future)),
            Err(AuthV2Error::FuturePublishTime)
        );

        let mut stale = bytes;
        set_i64(&mut stale, 93, 1_699_999_779);
        set_i64(&mut stale, 101, 1_699_999_778);
        assert_eq!(
            authenticate_pull_update_v2(auth(&stale)),
            Err(AuthV2Error::StalePublishTime)
        );
    }

    #[test]
    fn parser_owner_feed_and_config_account_are_authenticated() {
        let bytes = fixture();
        let mut case = auth(&bytes);
        case.update.owner = [0xf3; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::Parser(Error::WrongOwner))
        );

        let mut wrong_feed = bytes;
        wrong_feed[41] ^= 1;
        assert_eq!(
            authenticate_pull_update_v2(auth(&wrong_feed)),
            Err(AuthV2Error::Parser(Error::WrongFeed))
        );

        case = auth(&bytes);
        case.receiver_config.key = [0xf4; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongConfig)
        );
        case = auth(&bytes);
        case.receiver_config.owner = [0xf5; 32];
        assert_eq!(
            authenticate_pull_update_v2(case),
            Err(AuthV2Error::WrongConfig)
        );
    }
}
