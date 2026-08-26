//! Canonical transcripts compared by Trading and stateless V3 accelerators.
//!
//! These digests deliberately encode only already authenticated observations,
//! interpreted register/effect outputs, and invocation coordinates. They do
//! not authenticate accounts or artifacts and grant no state or CPI authority.

use core::convert::TryFrom;

use dclutch_core_contract::ContentId;
use sha2::{Digest, Sha256};

/// Domain for complete family request bytes.
pub const FAMILY_REQUEST_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-family-request:v3";
/// Domain for AccountProfile-ordered runtime observations.
pub const RUNTIME_OBSERVATION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-runtime-observations:v3";
/// Domain for one interpreted candidate register bank.
pub const CANDIDATE_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-candidate:v3";
/// Domain for one interpreted effect projection.
pub const EFFECT_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-effect:v3";
/// Domain for one selected action invocation.
pub const INVOCATION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-invocation:v3";

/// Stable refusal from canonical transcript construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowDigestErrorV3 {
    /// A slice length or coordinate exceeded its exact encoded `u32` width.
    CountOverflow,
    /// A route used an unknown role or route-kind tag.
    InvalidRouteTag,
    /// A route's optional item/witness presence grammar was noncanonical.
    InvalidRoutePresence,
    /// A supposedly read-only accelerator observation retained caller privileges.
    PrivilegedRuntimeObservation,
    /// SHA-256 produced the reserved all-zero content identity.
    ZeroDigest,
}

/// Result alias for canonical V3 transcript construction.
pub type Result<T> = core::result::Result<T, ShadowDigestErrorV3>;

/// One exact read-only runtime observation in AccountProfile logical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowRuntimeObservationV3<'a> {
    /// Account public key.
    pub key: [u8; 32],
    /// Account owner program.
    pub owner: [u8; 32],
    /// Observed lamports.
    pub lamports: u64,
    /// Exact observed account bytes.
    pub data: &'a [u8],
    /// Must be false: the accelerator receives no signer privilege for runtime state.
    pub signer: bool,
    /// Must be false: the accelerator receives runtime state read-only.
    pub writable: bool,
    /// Whether the top-level account is executable.
    pub executable: bool,
}

/// Canonical child role tag in the Shadow effect transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ShadowRouteRoleV3 {
    /// Market Core.
    Core = 0,
    /// Claims.
    Claims = 1,
    /// Resolution.
    Resolution = 3,
    /// Custody.
    Custody = 4,
}

/// Canonical invocation geometry tag in the Shadow effect transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ShadowRouteKindV3 {
    /// One fixed request/account frame.
    Once = 0,
    /// One fixed prefix plus the complete affine item tail.
    AffineOnce = 1,
    /// One invocation for one canonical item.
    Each = 2,
}

/// Exact adapter-resolved child route committed by the effect transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowResolvedRouteV3 {
    /// Authenticated child role.
    pub role: ShadowRouteRoleV3,
    /// Resolved invocation geometry.
    pub kind: ShadowRouteKindV3,
    /// Item ordinal, present only for [`ShadowRouteKindV3::Each`].
    pub item: Option<u32>,
    /// Fixed account-frame start.
    pub fixed_account_start: u16,
    /// Fixed account-frame count.
    pub fixed_account_count: u16,
    /// First expanded item-account coordinate.
    pub item_account_start: u32,
    /// Accounts in one repeated item subframe.
    pub item_account_count: u16,
    /// Distance between repeated item subframes.
    pub item_account_stride: u16,
    /// Number of repeated item subframes.
    pub repeated_item_count: u32,
    /// Offset in the projected request bank.
    pub request_offset: u32,
    /// Exact request bytes before any authenticated borrowed witness.
    pub request_len: u32,
    /// Exact optional `(offset, length)` in the complete family request.
    pub borrowed_witness: Option<(u32, u32)>,
}

impl ShadowResolvedRouteV3 {
    fn validate(self) -> Result<()> {
        let item_is_canonical = match self.kind {
            ShadowRouteKindV3::Each => self.item.is_some(),
            ShadowRouteKindV3::Once | ShadowRouteKindV3::AffineOnce => self.item.is_none(),
        };
        if !item_is_canonical {
            return Err(ShadowDigestErrorV3::InvalidRoutePresence);
        }
        Ok(())
    }
}

/// Complete interpreted effect projection before physical CPI or mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowEffectProjectionV3<'a> {
    /// Product-authoritative runtime tail count.
    pub tail_count: u32,
    /// Candidate lamports in AccountProfile logical order.
    pub output_lamports: &'a [u64],
    /// Exact projected request bank.
    pub request_bank: &'a [u8],
    /// Enabled, resolved child routes in canonical route/invocation order.
    pub routes: &'a [ShadowResolvedRouteV3],
}

/// Coordinates selecting one exact top-level invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowInvocationContextV3 {
    /// Current immutable release set.
    pub release_set: ContentId,
    /// Current logical Market.
    pub market: ContentId,
    /// Current Trading-owned root account.
    pub root: ContentId,
    /// Action-selected CapabilityProgramV3 content identity.
    pub capability_program: ContentId,
    /// Selected action value from CapabilityProgramSetV1.
    pub selected_action: u32,
    /// Digest of the exact complete family request.
    pub family_request_digest: ContentId,
    /// Digest of the exact root prestate.
    pub root_prestate_digest: ContentId,
}

/// Digest the exact complete family request.
pub fn family_request_digest_v3(bytes: &[u8]) -> Result<ContentId> {
    let mut hasher = begin(FAMILY_REQUEST_DIGEST_DOMAIN_V3);
    absorb_bytes(&mut hasher, bytes)?;
    finish(hasher)
}

/// Digest exact runtime observations in AccountProfile logical order.
pub fn runtime_observations_digest_v3(
    observations: &[ShadowRuntimeObservationV3<'_>],
) -> Result<ContentId> {
    let mut hasher = begin(RUNTIME_OBSERVATION_DIGEST_DOMAIN_V3);
    absorb_count(&mut hasher, observations.len())?;
    for observation in observations {
        if observation.signer || observation.writable {
            return Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation);
        }
        hasher.update(observation.key);
        hasher.update(observation.owner);
        hasher.update(observation.lamports.to_le_bytes());
        hasher.update([u8::from(observation.signer)]);
        hasher.update([u8::from(observation.writable)]);
        hasher.update([u8::from(observation.executable)]);
        hasher.update([0_u8]);
        absorb_bytes(&mut hasher, observation.data)?;
    }
    finish(hasher)
}

/// Digest one complete interpreted scalar/identity candidate bank.
pub fn candidate_digest_v3(
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<ContentId> {
    let mut hasher = begin(CANDIDATE_DIGEST_DOMAIN_V3);
    hasher.update(tail_count.to_le_bytes());
    absorb_count(&mut hasher, scalars.len())?;
    absorb_count(&mut hasher, identities.len())?;
    for scalar in scalars {
        hasher.update(scalar.to_le_bytes());
    }
    for identity in identities {
        hasher.update(identity);
    }
    finish(hasher)
}

/// Digest one complete interpreted effect projection before physical writes.
pub fn effect_digest_v3(projection: ShadowEffectProjectionV3<'_>) -> Result<ContentId> {
    let mut hasher = begin(EFFECT_DIGEST_DOMAIN_V3);
    hasher.update(projection.tail_count.to_le_bytes());
    absorb_count(&mut hasher, projection.output_lamports.len())?;
    for lamports in projection.output_lamports {
        hasher.update(lamports.to_le_bytes());
    }
    absorb_bytes(&mut hasher, projection.request_bank)?;
    absorb_count(&mut hasher, projection.routes.len())?;
    for route in projection.routes {
        route.validate()?;
        hasher.update([route.role as u8]);
        hasher.update([route.kind as u8]);
        match route.item {
            Some(item) => {
                hasher.update([1_u8]);
                hasher.update(item.to_le_bytes());
            }
            None => hasher.update([0_u8; 5]),
        }
        hasher.update(route.fixed_account_start.to_le_bytes());
        hasher.update(route.fixed_account_count.to_le_bytes());
        hasher.update(route.item_account_start.to_le_bytes());
        hasher.update(route.item_account_count.to_le_bytes());
        hasher.update(route.item_account_stride.to_le_bytes());
        hasher.update(route.repeated_item_count.to_le_bytes());
        hasher.update(route.request_offset.to_le_bytes());
        hasher.update(route.request_len.to_le_bytes());
        match route.borrowed_witness {
            Some((offset, len)) => {
                hasher.update([1_u8]);
                hasher.update(offset.to_le_bytes());
                hasher.update(len.to_le_bytes());
            }
            None => hasher.update([0_u8; 9]),
        }
    }
    finish(hasher)
}

/// Digest one action-selected invocation context.
pub fn invocation_context_digest_v3(context: ShadowInvocationContextV3) -> Result<ContentId> {
    let mut hasher = begin(INVOCATION_DIGEST_DOMAIN_V3);
    hasher.update(context.release_set.as_bytes());
    hasher.update(context.market.as_bytes());
    hasher.update(context.root.as_bytes());
    hasher.update(context.capability_program.as_bytes());
    hasher.update(context.selected_action.to_le_bytes());
    hasher.update(context.family_request_digest.as_bytes());
    hasher.update(context.root_prestate_digest.as_bytes());
    finish(hasher)
}

fn begin(domain: &[u8]) -> Sha256 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0_u8]);
    hasher
}

fn absorb_count(hasher: &mut Sha256, count: usize) -> Result<()> {
    let count = u32::try_from(count).map_err(|_| ShadowDigestErrorV3::CountOverflow)?;
    hasher.update(count.to_le_bytes());
    Ok(())
}

fn absorb_bytes(hasher: &mut Sha256, bytes: &[u8]) -> Result<()> {
    absorb_count(hasher, bytes.len())?;
    hasher.update(bytes);
    Ok(())
}

fn finish(hasher: Sha256) -> Result<ContentId> {
    ContentId::new(hasher.finalize().into()).map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero")
    }

    fn observation<'a>(key: u8, data: &'a [u8]) -> ShadowRuntimeObservationV3<'a> {
        ShadowRuntimeObservationV3 {
            key: [key; 32],
            owner: [9; 32],
            lamports: 10,
            data,
            signer: false,
            writable: false,
            executable: false,
        }
    }

    fn route() -> ShadowResolvedRouteV3 {
        ShadowResolvedRouteV3 {
            role: ShadowRouteRoleV3::Custody,
            kind: ShadowRouteKindV3::Each,
            item: Some(2),
            fixed_account_start: 4,
            fixed_account_count: 5,
            item_account_start: 6,
            item_account_count: 7,
            item_account_stride: 8,
            repeated_item_count: 1,
            request_offset: 9,
            request_len: 10,
            borrowed_witness: Some((11, 12)),
        }
    }

    #[test]
    fn domains_and_runtime_order_are_distinct() {
        let first = observation(1, b"first");
        let second = observation(2, b"second");
        let ordered = runtime_observations_digest_v3(&[first, second]).expect("runtime");
        let swapped = runtime_observations_digest_v3(&[second, first]).expect("runtime");
        assert_ne!(ordered, swapped);
        assert_ne!(
            ordered,
            family_request_digest_v3(b"firstsecond").expect("family")
        );
        let substituted = observation(1, b"First");
        assert_ne!(
            runtime_observations_digest_v3(&[first]).expect("runtime"),
            runtime_observations_digest_v3(&[substituted]).expect("runtime")
        );
    }

    #[test]
    fn runtime_transcript_refuses_any_forwarded_privilege() {
        let mut privileged = observation(1, b"state");
        privileged.signer = true;
        assert_eq!(
            runtime_observations_digest_v3(&[privileged]),
            Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation)
        );
        privileged.signer = false;
        privileged.writable = true;
        assert_eq!(
            runtime_observations_digest_v3(&[privileged]),
            Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation)
        );
    }

    #[test]
    fn candidate_binds_dimensions_order_and_tail() {
        let canonical = candidate_digest_v3(3, &[1, 2], &[[3; 32]]).expect("candidate");
        assert_ne!(
            canonical,
            candidate_digest_v3(4, &[1, 2], &[[3; 32]]).expect("candidate")
        );
        assert_ne!(
            canonical,
            candidate_digest_v3(3, &[2, 1], &[[3; 32]]).expect("candidate")
        );
    }

    #[test]
    fn effect_binds_banks_routes_and_presence() {
        let canonical = effect_digest_v3(ShadowEffectProjectionV3 {
            tail_count: 3,
            output_lamports: &[4, 5],
            request_bank: b"request",
            routes: &[route()],
        })
        .expect("effect");
        let mut changed = route();
        changed.request_len = 11;
        assert_ne!(
            canonical,
            effect_digest_v3(ShadowEffectProjectionV3 {
                tail_count: 3,
                output_lamports: &[4, 5],
                request_bank: b"request",
                routes: &[changed],
            })
            .expect("effect")
        );
        changed.kind = ShadowRouteKindV3::Once;
        assert_eq!(
            effect_digest_v3(ShadowEffectProjectionV3 {
                tail_count: 3,
                output_lamports: &[4, 5],
                request_bank: b"request",
                routes: &[changed],
            }),
            Err(ShadowDigestErrorV3::InvalidRoutePresence)
        );
    }

    #[test]
    fn invocation_binds_action_request_and_prestate() {
        let canonical = ShadowInvocationContextV3 {
            release_set: id(1),
            market: id(2),
            root: id(3),
            capability_program: id(4),
            selected_action: 5,
            family_request_digest: id(6),
            root_prestate_digest: id(7),
        };
        let first = invocation_context_digest_v3(canonical).expect("invocation");
        let mut changed = canonical;
        changed.selected_action = 6;
        assert_ne!(
            first,
            invocation_context_digest_v3(changed).expect("invocation")
        );
    }
}
