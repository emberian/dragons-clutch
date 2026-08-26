//! Fixed generic transport for one stateless Shadow-AOT comparison.
//!
//! Trading remains the sole interpreter, CPI caller, and state writer. It
//! supplies the complete authenticated family request and a canonical runtime
//! account slice to a read-only accelerator, then accepts an acknowledgement
//! only when the accelerator binds the same artifact tuple and the exact
//! interpreted candidate/effect digests. This contract contains no family tag,
//! account semantics, hashing implementation, or authority policy.

use core::convert::TryInto;

use dclutch_core_contract::ContentId;

/// Finalized-record schema preimage for [`ShadowRequestV3`].
pub const SHADOW_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/shadow-aot-request-v3";
/// SHA-256 of [`SHADOW_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const SHADOW_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0x06, 0x4e, 0x3f, 0xa6, 0x14, 0x7c, 0xc1, 0x62, 0x8d, 0x9f, 0xb3, 0xe2, 0x79, 0x14, 0x37, 0x7d,
    0xaf, 0x9a, 0xf4, 0x92, 0xe0, 0xeb, 0x41, 0xbd, 0x0c, 0x62, 0x0b, 0x27, 0x9c, 0x06, 0x2e, 0xa3,
];
/// Finalized-record schema preimage for [`ShadowAckV3`].
pub const SHADOW_ACK_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/shadow-aot-ack-v3";
/// SHA-256 of [`SHADOW_ACK_SCHEMA_PREIMAGE_V3`].
pub const SHADOW_ACK_SCHEMA_ID_V3: [u8; 32] = [
    0x98, 0xc4, 0x6e, 0xda, 0x65, 0x90, 0x0b, 0xfa, 0xe7, 0xa6, 0xa5, 0x61, 0x59, 0x86, 0x3b, 0x51,
    0x93, 0x9f, 0x5b, 0x25, 0xfb, 0xce, 0x40, 0x1b, 0x3a, 0x41, 0x24, 0x34, 0xba, 0x9a, 0xcc, 0xc7,
];
/// Exact request magic.
pub const SHADOW_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLTSR03";
/// Exact acknowledgement magic.
pub const SHADOW_ACK_MAGIC_V3: [u8; 8] = *b"DCLTSA03";
/// Implemented wire version.
pub const SHADOW_VERSION_V3: u16 = 3;
/// Read-only account-slice Shadow profile.
pub const SHADOW_PROFILE_V3: u16 = 1;
/// Fixed request bytes before the exact family request.
pub const SHADOW_REQUEST_HEADER_BYTES_V3: usize = 624;
/// Exact fixed acknowledgement width.
pub const SHADOW_ACK_BYTES_V3: usize = 528;
/// Caller-authority PDA in the accelerator CPI frame.
pub const SHADOW_CALLER_AUTHORITY_ACCOUNT_V3: usize = 0;
/// Current activated release-set cache in the accelerator CPI frame.
pub const SHADOW_ACTIVATION_ACCOUNT_V3: usize = 1;
/// Current Registry program in the accelerator CPI frame.
pub const SHADOW_REGISTRY_PROGRAM_ACCOUNT_V3: usize = 2;
/// Current release-selected Trading program in the accelerator CPI frame.
pub const SHADOW_TRADING_PROGRAM_ACCOUNT_V3: usize = 3;
/// Current Trading ProgramData in the accelerator CPI frame.
pub const SHADOW_TRADING_PROGRAMDATA_ACCOUNT_V3: usize = 4;
/// Authenticated accelerator ProgramData in the accelerator CPI frame.
pub const SHADOW_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3: usize = 5;
/// Fixed read-only prefix before AccountProfile-ordered runtime observations.
pub const SHADOW_RUNTIME_ACCOUNTS_START_V3: usize = 6;

const VERSION_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const REQUEST_RESERVED_OFFSET: usize = 12;
const REQUEST_RESERVED_BYTES: usize = 4;
const RELEASE_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const ROOT_OFFSET: usize = 80;
const REGISTRY_OFFSET: usize = 112;
const TRADING_OFFSET: usize = 144;
const ACCELERATOR_OFFSET: usize = 176;
const CAPABILITY_OFFSET: usize = 208;
const ACCOUNT_PROFILE_OFFSET: usize = 240;
const REQUEST_PROFILE_OFFSET: usize = 272;
const TRANSITION_OFFSET: usize = 304;
const EFFECT_OFFSET: usize = 336;
const STRATEGY_OFFSET: usize = 368;
const CERTIFICATE_OFFSET: usize = 400;
const INVOCATION_OFFSET: usize = 432;
const RUNTIME_DIGEST_OFFSET: usize = 464;
const FAMILY_DIGEST_OFFSET: usize = 496;
const CANDIDATE_DIGEST_OFFSET: usize = 528;
const EFFECT_DIGEST_OFFSET: usize = 560;
const TAIL_COUNT_OFFSET: usize = 592;
const ACCOUNT_COUNT_OFFSET: usize = 596;
const SCALAR_COUNT_OFFSET: usize = 600;
const IDENTITY_COUNT_OFFSET: usize = 604;
const FAMILY_BYTES_OFFSET: usize = 608;
const REQUEST_TAIL_RESERVED_OFFSET: usize = 612;
const REQUEST_TAIL_RESERVED_BYTES: usize = 12;

const ACK_DISPOSITION_OFFSET: usize = 12;
const ACK_HEADER_RESERVED_OFFSET: usize = 13;
const ACK_HEADER_RESERVED_BYTES: usize = 3;
const ACK_REQUEST_DIGEST_OFFSET: usize = 16;
const ACK_RELEASE_OFFSET: usize = 48;
const ACK_MARKET_OFFSET: usize = 80;
const ACK_ACCELERATOR_OFFSET: usize = 112;
const ACK_CAPABILITY_OFFSET: usize = 144;
const ACK_ACCOUNT_PROFILE_OFFSET: usize = 176;
const ACK_REQUEST_PROFILE_OFFSET: usize = 208;
const ACK_TRANSITION_OFFSET: usize = 240;
const ACK_EFFECT_OFFSET: usize = 272;
const ACK_STRATEGY_OFFSET: usize = 304;
const ACK_CERTIFICATE_OFFSET: usize = 336;
const ACK_INVOCATION_OFFSET: usize = 368;
const ACK_CANDIDATE_DIGEST_OFFSET: usize = 400;
const ACK_EFFECT_DIGEST_OFFSET: usize = 432;
const ACK_RUNTIME_DIGEST_OFFSET: usize = 464;
const ACK_FAMILY_DIGEST_OFFSET: usize = 496;

const ACK_REFUSED: u8 = 0;
const ACK_ACCEPTED: u8 = 1;

/// Stable hostile-decode or binding refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowErrorV3 {
    /// Exact count-derived wire width differed.
    InvalidLength,
    /// Magic selected another transport.
    InvalidMagic,
    /// Version or profile is unsupported.
    UnsupportedProfile,
    /// Reserved bytes were nonzero.
    NonCanonicalReserved,
    /// A required identity/digest was zero.
    ZeroIdentity,
    /// Account/register geometry was empty or overflowed.
    InvalidGeometry,
    /// Family request digest or caller-supplied binding differed.
    BindingMismatch,
    /// Acknowledgement disposition was unknown.
    UnknownDisposition,
}

/// Result alias for Shadow V3 transport.
pub type Result<T> = core::result::Result<T, ShadowErrorV3>;

/// Exact content-selected interpreter and accelerator tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowArtifactTupleV3 {
    /// CapabilityProgramV3 content identity.
    pub capability_program: ContentId,
    /// AccountProfile content identity.
    pub account_profile: ContentId,
    /// RequestProfile content identity.
    pub request_profile: ContentId,
    /// Transition program content identity.
    pub transition: ContentId,
    /// EffectProgram content identity.
    pub effect: ContentId,
    /// ExecutionStrategy content identity.
    pub strategy: ContentId,
    /// Translation-validation Certificate content identity.
    pub certificate: ContentId,
}

/// Exact interpreter observations compared by the stateless accelerator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowExecutionDigestsV3 {
    /// Digest of canonical runtime account observations in AccountProfile order.
    pub runtime_observations: ContentId,
    /// Digest of the complete family request.
    pub family_request: ContentId,
    /// Digest of the complete interpreted candidate register bank.
    pub interpreted_candidate: ContentId,
    /// Digest of the complete interpreted effect projection before CPI.
    pub interpreted_effect: ContentId,
}

/// Exact runtime dimensions bound into one Shadow comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowRuntimeShapeV3 {
    /// Product-authoritative semantic tail count.
    pub tail_count: u32,
    /// Exact AccountProfile-expanded runtime account count.
    pub account_count: u32,
    /// Exact runtime scalar-bank count.
    pub scalar_count: u32,
    /// Exact runtime identity-bank count.
    pub identity_count: u32,
}

impl ShadowRuntimeShapeV3 {
    /// Refuse empty account geometry and empty combined register geometry.
    pub fn validate(self) -> Result<()> {
        if self.account_count == 0 || (self.scalar_count == 0 && self.identity_count == 0) {
            Err(ShadowErrorV3::InvalidGeometry)
        } else {
            Ok(())
        }
    }
}

/// Borrowed complete Shadow request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowRequestV3<'a> {
    /// Current immutable release set.
    pub release_set: ContentId,
    /// Current logical Market.
    pub market: ContentId,
    /// Current Trading root account.
    pub root: ContentId,
    /// Current Registry program.
    pub registry_program: ContentId,
    /// Current release-selected Trading program.
    pub trading_program: ContentId,
    /// Current authenticated accelerator program.
    pub accelerator_program: ContentId,
    /// Exact selected interpreter/AOT artifact tuple.
    pub artifacts: ShadowArtifactTupleV3,
    /// Digest binding this one invocation and action.
    pub invocation_context: ContentId,
    /// Exact interpreted observations to reproduce.
    pub digests: ShadowExecutionDigestsV3,
    /// Exact runtime dimensions.
    pub shape: ShadowRuntimeShapeV3,
    /// Complete family request, including any proof witness.
    pub family_request: &'a [u8],
}

impl<'a> ShadowRequestV3<'a> {
    /// Hostile-decode one exact request and trailing family request.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < SHADOW_REQUEST_HEADER_BYTES_V3 {
            return Err(ShadowErrorV3::InvalidLength);
        }
        require_magic(bytes, &SHADOW_REQUEST_MAGIC_V3)?;
        require_profile(bytes)?;
        require_zero(bytes, REQUEST_RESERVED_OFFSET, REQUEST_RESERVED_BYTES)?;
        require_zero(
            bytes,
            REQUEST_TAIL_RESERVED_OFFSET,
            REQUEST_TAIL_RESERVED_BYTES,
        )?;
        let family_len = usize::try_from(read_u32(bytes, FAMILY_BYTES_OFFSET)?)
            .map_err(|_| ShadowErrorV3::InvalidLength)?;
        let expected = SHADOW_REQUEST_HEADER_BYTES_V3
            .checked_add(family_len)
            .ok_or(ShadowErrorV3::InvalidLength)?;
        if bytes.len() != expected || family_len == 0 {
            return Err(ShadowErrorV3::InvalidLength);
        }
        let family_request = bytes
            .get(SHADOW_REQUEST_HEADER_BYTES_V3..)
            .ok_or(ShadowErrorV3::InvalidLength)?;
        let request = Self {
            release_set: content(bytes, RELEASE_OFFSET)?,
            market: content(bytes, MARKET_OFFSET)?,
            root: content(bytes, ROOT_OFFSET)?,
            registry_program: content(bytes, REGISTRY_OFFSET)?,
            trading_program: content(bytes, TRADING_OFFSET)?,
            accelerator_program: content(bytes, ACCELERATOR_OFFSET)?,
            artifacts: ShadowArtifactTupleV3 {
                capability_program: content(bytes, CAPABILITY_OFFSET)?,
                account_profile: content(bytes, ACCOUNT_PROFILE_OFFSET)?,
                request_profile: content(bytes, REQUEST_PROFILE_OFFSET)?,
                transition: content(bytes, TRANSITION_OFFSET)?,
                effect: content(bytes, EFFECT_OFFSET)?,
                strategy: content(bytes, STRATEGY_OFFSET)?,
                certificate: content(bytes, CERTIFICATE_OFFSET)?,
            },
            invocation_context: content(bytes, INVOCATION_OFFSET)?,
            digests: ShadowExecutionDigestsV3 {
                runtime_observations: content(bytes, RUNTIME_DIGEST_OFFSET)?,
                family_request: content(bytes, FAMILY_DIGEST_OFFSET)?,
                interpreted_candidate: content(bytes, CANDIDATE_DIGEST_OFFSET)?,
                interpreted_effect: content(bytes, EFFECT_DIGEST_OFFSET)?,
            },
            shape: ShadowRuntimeShapeV3 {
                tail_count: read_u32(bytes, TAIL_COUNT_OFFSET)?,
                account_count: read_u32(bytes, ACCOUNT_COUNT_OFFSET)?,
                scalar_count: read_u32(bytes, SCALAR_COUNT_OFFSET)?,
                identity_count: read_u32(bytes, IDENTITY_COUNT_OFFSET)?,
            },
            family_request,
        };
        request.shape.validate()?;
        Ok(request)
    }

    /// Encode into an exact caller-owned buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        self.shape.validate()?;
        let expected = SHADOW_REQUEST_HEADER_BYTES_V3
            .checked_add(self.family_request.len())
            .ok_or(ShadowErrorV3::InvalidLength)?;
        if output.len() != expected || self.family_request.is_empty() {
            return Err(ShadowErrorV3::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &SHADOW_REQUEST_MAGIC_V3)?;
        put_u16(output, VERSION_OFFSET, SHADOW_VERSION_V3)?;
        put_u16(output, PROFILE_OFFSET, SHADOW_PROFILE_V3)?;
        for (offset, value) in [
            (RELEASE_OFFSET, self.release_set),
            (MARKET_OFFSET, self.market),
            (ROOT_OFFSET, self.root),
            (REGISTRY_OFFSET, self.registry_program),
            (TRADING_OFFSET, self.trading_program),
            (ACCELERATOR_OFFSET, self.accelerator_program),
            (CAPABILITY_OFFSET, self.artifacts.capability_program),
            (ACCOUNT_PROFILE_OFFSET, self.artifacts.account_profile),
            (REQUEST_PROFILE_OFFSET, self.artifacts.request_profile),
            (TRANSITION_OFFSET, self.artifacts.transition),
            (EFFECT_OFFSET, self.artifacts.effect),
            (STRATEGY_OFFSET, self.artifacts.strategy),
            (CERTIFICATE_OFFSET, self.artifacts.certificate),
            (INVOCATION_OFFSET, self.invocation_context),
            (RUNTIME_DIGEST_OFFSET, self.digests.runtime_observations),
            (FAMILY_DIGEST_OFFSET, self.digests.family_request),
            (CANDIDATE_DIGEST_OFFSET, self.digests.interpreted_candidate),
            (EFFECT_DIGEST_OFFSET, self.digests.interpreted_effect),
        ] {
            put(output, offset, value.as_bytes())?;
        }
        put_u32(output, TAIL_COUNT_OFFSET, self.shape.tail_count)?;
        put_u32(output, ACCOUNT_COUNT_OFFSET, self.shape.account_count)?;
        put_u32(output, SCALAR_COUNT_OFFSET, self.shape.scalar_count)?;
        put_u32(output, IDENTITY_COUNT_OFFSET, self.shape.identity_count)?;
        put_u32(
            output,
            FAMILY_BYTES_OFFSET,
            u32::try_from(self.family_request.len()).map_err(|_| ShadowErrorV3::InvalidLength)?,
        )?;
        put(output, SHADOW_REQUEST_HEADER_BYTES_V3, self.family_request)?;
        Ok(())
    }

    /// Exact encoded request width.
    pub fn encoded_len(self) -> Result<usize> {
        SHADOW_REQUEST_HEADER_BYTES_V3
            .checked_add(self.family_request.len())
            .ok_or(ShadowErrorV3::InvalidLength)
    }

    /// Require independently computed family request and runtime digests.
    pub fn validate_observations(
        self,
        family_request_digest: ContentId,
        runtime_observation_digest: ContentId,
    ) -> Result<()> {
        if self.digests.family_request != family_request_digest
            || self.digests.runtime_observations != runtime_observation_digest
        {
            Err(ShadowErrorV3::BindingMismatch)
        } else {
            Ok(())
        }
    }
}

/// Accelerator acknowledgement disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowDispositionV3 {
    /// Semantic validation refused; no candidate is authorized.
    Refused,
    /// The accelerator reproduced the exact interpreted observations.
    Accepted,
}

/// Fixed typed acknowledgement returned by a stateless accelerator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowAckV3 {
    disposition: ShadowDispositionV3,
    request_digest: ContentId,
    request: ShadowRequestBindingV3,
}

/// Exact request coordinates repeated by [`ShadowAckV3`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowRequestBindingV3 {
    /// Current release set.
    pub release_set: ContentId,
    /// Current Market.
    pub market: ContentId,
    /// Authenticated accelerator program.
    pub accelerator_program: ContentId,
    /// Exact artifact tuple.
    pub artifacts: ShadowArtifactTupleV3,
    /// Invocation-context digest.
    pub invocation_context: ContentId,
    /// Exact interpreted digests.
    pub digests: ShadowExecutionDigestsV3,
}

impl ShadowRequestBindingV3 {
    /// Bind the acknowledgement coordinates from one decoded request.
    pub const fn from_request(request: ShadowRequestV3<'_>) -> Self {
        Self {
            release_set: request.release_set,
            market: request.market,
            accelerator_program: request.accelerator_program,
            artifacts: request.artifacts,
            invocation_context: request.invocation_context,
            digests: request.digests,
        }
    }
}

impl ShadowAckV3 {
    /// Construct one exact accepted acknowledgement.
    pub const fn accepted(request: ShadowRequestV3<'_>, request_digest: ContentId) -> Self {
        Self {
            disposition: ShadowDispositionV3::Accepted,
            request_digest,
            request: ShadowRequestBindingV3::from_request(request),
        }
    }

    /// Construct one exact semantic refusal.
    pub const fn refused(request: ShadowRequestV3<'_>, request_digest: ContentId) -> Self {
        Self {
            disposition: ShadowDispositionV3::Refused,
            request_digest,
            request: ShadowRequestBindingV3::from_request(request),
        }
    }

    /// Hostile-decode one exact acknowledgement.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SHADOW_ACK_BYTES_V3 {
            return Err(ShadowErrorV3::InvalidLength);
        }
        require_magic(bytes, &SHADOW_ACK_MAGIC_V3)?;
        require_profile(bytes)?;
        require_zero(bytes, ACK_HEADER_RESERVED_OFFSET, ACK_HEADER_RESERVED_BYTES)?;
        let disposition = match byte(bytes, ACK_DISPOSITION_OFFSET)? {
            ACK_REFUSED => ShadowDispositionV3::Refused,
            ACK_ACCEPTED => ShadowDispositionV3::Accepted,
            _ => return Err(ShadowErrorV3::UnknownDisposition),
        };
        Ok(Self {
            disposition,
            request_digest: content(bytes, ACK_REQUEST_DIGEST_OFFSET)?,
            request: ShadowRequestBindingV3 {
                release_set: content(bytes, ACK_RELEASE_OFFSET)?,
                market: content(bytes, ACK_MARKET_OFFSET)?,
                accelerator_program: content(bytes, ACK_ACCELERATOR_OFFSET)?,
                artifacts: ShadowArtifactTupleV3 {
                    capability_program: content(bytes, ACK_CAPABILITY_OFFSET)?,
                    account_profile: content(bytes, ACK_ACCOUNT_PROFILE_OFFSET)?,
                    request_profile: content(bytes, ACK_REQUEST_PROFILE_OFFSET)?,
                    transition: content(bytes, ACK_TRANSITION_OFFSET)?,
                    effect: content(bytes, ACK_EFFECT_OFFSET)?,
                    strategy: content(bytes, ACK_STRATEGY_OFFSET)?,
                    certificate: content(bytes, ACK_CERTIFICATE_OFFSET)?,
                },
                invocation_context: content(bytes, ACK_INVOCATION_OFFSET)?,
                digests: ShadowExecutionDigestsV3 {
                    interpreted_candidate: content(bytes, ACK_CANDIDATE_DIGEST_OFFSET)?,
                    interpreted_effect: content(bytes, ACK_EFFECT_DIGEST_OFFSET)?,
                    runtime_observations: content(bytes, ACK_RUNTIME_DIGEST_OFFSET)?,
                    family_request: content(bytes, ACK_FAMILY_DIGEST_OFFSET)?,
                },
            },
        })
    }

    /// Encode the exact fixed acknowledgement.
    pub fn to_bytes(self) -> Result<[u8; SHADOW_ACK_BYTES_V3]> {
        let mut output = [0_u8; SHADOW_ACK_BYTES_V3];
        put(&mut output, 0, &SHADOW_ACK_MAGIC_V3)?;
        put_u16(&mut output, VERSION_OFFSET, SHADOW_VERSION_V3)?;
        put_u16(&mut output, PROFILE_OFFSET, SHADOW_PROFILE_V3)?;
        *output
            .get_mut(ACK_DISPOSITION_OFFSET)
            .ok_or(ShadowErrorV3::InvalidLength)? = match self.disposition {
            ShadowDispositionV3::Refused => ACK_REFUSED,
            ShadowDispositionV3::Accepted => ACK_ACCEPTED,
        };
        for (offset, value) in [
            (ACK_REQUEST_DIGEST_OFFSET, self.request_digest),
            (ACK_RELEASE_OFFSET, self.request.release_set),
            (ACK_MARKET_OFFSET, self.request.market),
            (ACK_ACCELERATOR_OFFSET, self.request.accelerator_program),
            (
                ACK_CAPABILITY_OFFSET,
                self.request.artifacts.capability_program,
            ),
            (
                ACK_ACCOUNT_PROFILE_OFFSET,
                self.request.artifacts.account_profile,
            ),
            (
                ACK_REQUEST_PROFILE_OFFSET,
                self.request.artifacts.request_profile,
            ),
            (ACK_TRANSITION_OFFSET, self.request.artifacts.transition),
            (ACK_EFFECT_OFFSET, self.request.artifacts.effect),
            (ACK_STRATEGY_OFFSET, self.request.artifacts.strategy),
            (ACK_CERTIFICATE_OFFSET, self.request.artifacts.certificate),
            (ACK_INVOCATION_OFFSET, self.request.invocation_context),
            (
                ACK_CANDIDATE_DIGEST_OFFSET,
                self.request.digests.interpreted_candidate,
            ),
            (
                ACK_EFFECT_DIGEST_OFFSET,
                self.request.digests.interpreted_effect,
            ),
            (
                ACK_RUNTIME_DIGEST_OFFSET,
                self.request.digests.runtime_observations,
            ),
            (
                ACK_FAMILY_DIGEST_OFFSET,
                self.request.digests.family_request,
            ),
        ] {
            put(&mut output, offset, value.as_bytes())?;
        }
        Ok(output)
    }

    /// Require exact producer-bound request coordinates.
    pub fn validate_for(
        self,
        request: ShadowRequestV3<'_>,
        request_digest: ContentId,
        accelerator_program: ContentId,
    ) -> Result<()> {
        if self.request_digest != request_digest
            || self.request != ShadowRequestBindingV3::from_request(request)
            || self.request.accelerator_program != accelerator_program
        {
            Err(ShadowErrorV3::BindingMismatch)
        } else {
            Ok(())
        }
    }

    /// Accepted or refused disposition.
    pub const fn disposition(self) -> ShadowDispositionV3 {
        self.disposition
    }

    /// Request digest acknowledged by the accelerator.
    pub const fn request_digest(self) -> ContentId {
        self.request_digest
    }
}

fn require_magic(bytes: &[u8], magic: &[u8; 8]) -> Result<()> {
    if bytes.get(..8) == Some(magic.as_slice()) {
        Ok(())
    } else {
        Err(ShadowErrorV3::InvalidMagic)
    }
}

fn require_profile(bytes: &[u8]) -> Result<()> {
    if read_u16(bytes, VERSION_OFFSET)? == SHADOW_VERSION_V3
        && read_u16(bytes, PROFILE_OFFSET)? == SHADOW_PROFILE_V3
    {
        Ok(())
    } else {
        Err(ShadowErrorV3::UnsupportedProfile)
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset
        .checked_add(width)
        .ok_or(ShadowErrorV3::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(ShadowErrorV3::InvalidLength)?
        .iter()
        .all(|value| *value == 0)
    {
        Ok(())
    } else {
        Err(ShadowErrorV3::NonCanonicalReserved)
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ShadowErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = offset.checked_add(2).ok_or(ShadowErrorV3::InvalidLength)?;
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(ShadowErrorV3::InvalidLength)?
            .try_into()
            .map_err(|_| ShadowErrorV3::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or(ShadowErrorV3::InvalidLength)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(ShadowErrorV3::InvalidLength)?
            .try_into()
            .map_err(|_| ShadowErrorV3::InvalidLength)?,
    ))
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    let end = offset.checked_add(32).ok_or(ShadowErrorV3::InvalidLength)?;
    ContentId::decode(bytes.get(offset..end).ok_or(ShadowErrorV3::InvalidLength)?)
        .map_err(|_| ShadowErrorV3::ZeroIdentity)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ShadowErrorV3::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(ShadowErrorV3::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content")
    }

    fn request<'a>(family: &'a [u8]) -> ShadowRequestV3<'a> {
        ShadowRequestV3 {
            release_set: id(1),
            market: id(2),
            root: id(3),
            registry_program: id(4),
            trading_program: id(5),
            accelerator_program: id(6),
            artifacts: ShadowArtifactTupleV3 {
                capability_program: id(7),
                account_profile: id(8),
                request_profile: id(9),
                transition: id(10),
                effect: id(11),
                strategy: id(12),
                certificate: id(13),
            },
            invocation_context: id(14),
            digests: ShadowExecutionDigestsV3 {
                runtime_observations: id(15),
                family_request: id(16),
                interpreted_candidate: id(17),
                interpreted_effect: id(18),
            },
            shape: ShadowRuntimeShapeV3 {
                tail_count: 19,
                account_count: 20,
                scalar_count: 21,
                identity_count: 22,
            },
            family_request: family,
        }
    }

    #[test]
    fn request_and_ack_round_trip_exact_artifact_tuple() {
        let family = [23_u8; 192];
        let request = request(&family);
        let mut bytes = vec![0_u8; request.encoded_len().expect("width")];
        request.encode_into(&mut bytes).expect("encode");
        let decoded = ShadowRequestV3::decode(&bytes).expect("decode");
        assert_eq!(decoded, request);

        let ack = ShadowAckV3::accepted(decoded, id(24));
        let bytes = ack.to_bytes().expect("encode ack");
        let decoded_ack = ShadowAckV3::decode(&bytes).expect("ack");
        assert_eq!(decoded_ack, ack);
        assert_eq!(decoded_ack.validate_for(request, id(24), id(6)), Ok(()));
    }

    #[test]
    fn truncation_padding_reserved_and_tuple_substitution_refuse() {
        let family = [25_u8; 128];
        let request = request(&family);
        let mut bytes = vec![0_u8; request.encoded_len().expect("width")];
        request.encode_into(&mut bytes).expect("encode");
        assert_eq!(
            ShadowRequestV3::decode(bytes.get(..bytes.len() - 1).expect("short")),
            Err(ShadowErrorV3::InvalidLength)
        );
        bytes.push(0);
        assert_eq!(
            ShadowRequestV3::decode(&bytes),
            Err(ShadowErrorV3::InvalidLength)
        );
        bytes.pop();
        *bytes
            .get_mut(REQUEST_TAIL_RESERVED_OFFSET)
            .expect("reserved") = 1;
        assert_eq!(
            ShadowRequestV3::decode(&bytes),
            Err(ShadowErrorV3::NonCanonicalReserved)
        );

        let ack = ShadowAckV3::accepted(request, id(26));
        assert_eq!(
            ack.validate_for(request, id(26), id(27)),
            Err(ShadowErrorV3::BindingMismatch)
        );
    }
}
