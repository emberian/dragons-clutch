//! Caller-authority derivation for projected child routes.
//!
//! The Hot executor derives one `CallerAuthority` PDA per child invocation
//! from the invocation's exact projected request bytes; the frame's first
//! account must be that PDA (`custody_composition_v3::prepare`,
//! `claims_composition_v3::route_authority` are the on-chain authorities this
//! mirrors). The builder derives the same addresses from the same requests, so
//! a family campaign never states one.
//!
//! The context seed is request-kind-specific and comes from inside the
//! request, never from the family envelope: Custody uses the request's own
//! `context`; each Claims kind names its own field. Kinds not yet dispatched
//! here are a named boundary, not a silent default.

use dclutch_claims_svm::{
    affine_batch_v2::{AFFINE_BATCH_PLAN_MAGIC_V2, AffineBatchPlanV2},
    founding_v5::{CLAIMS_FOUNDING_REQUEST_MAGIC_V5, ClaimsFoundingRequestV5},
    protocol_position_v2::{PROTOCOL_POSITION_REQUEST_MAGIC_V2, ProtocolPositionRequestV2},
    signed_delta_v3::{SIGNED_DELTA_PLAN_MAGIC_V3, SignedDeltaPlanV3},
    sparse_native_transfer_v1::{SPARSE_NATIVE_TRANSFER_MAGIC_V1, SparseNativeTransferV1},
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CustodyRequestLayoutV1, DELEGATED_CUSTODY_REQUEST_MAGIC_V2, DelegatedCustodyRequestLayoutV2,
};
use dclutch_effect_kernel::v2::FixedRole;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use sha2::{Digest, Sha256};
use solana_program::pubkey::Pubkey;

use crate::{BuilderError, registers::DerivedInvocationV1};

/// One derived caller authority and the coordinate that must carry it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedAuthorityV1 {
    /// Logical coordinate of the child frame's first account.
    pub coordinate: usize,
    /// The PDA the Hot executor will derive for this invocation.
    pub authority: Pubkey,
    /// SHA-256 of the exact projected child request.
    pub request_digest: [u8; 32],
}

/// Derive one invocation's caller authority, or `None` for roles that carry no
/// authority coordinate at the frame start (Core).
pub fn derive_authority(
    invocation: &DerivedInvocationV1,
    release_set: [u8; 32],
    trading_program: Pubkey,
) -> Result<Option<DerivedAuthorityV1>, BuilderError> {
    let request = invocation.request.as_slice();
    let request_digest: [u8; 32] = Sha256::digest(request).into();
    let (market, context) = match invocation.resolved.role {
        FixedRole::Core | FixedRole::Resolution => return Ok(None),
        FixedRole::Custody => custody_market_and_context(request)?,
        FixedRole::Claims => claims_context(request)?,
    };
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).map_err(|_| BuilderError::UnsupportedRoute)?,
        market,
        ExecutionRoleV1::Trading,
        context,
        request_digest,
    )
    .map_err(|_| BuilderError::UnsupportedRoute)?;
    let authority = Pubkey::find_program_address(&seeds.as_slices(), &trading_program).0;
    Ok(Some(DerivedAuthorityV1 {
        coordinate: usize::from(invocation.resolved.fixed_account_start),
        authority,
        request_digest,
    }))
}

/// Read the Custody request's market and context at their layout offsets.
///
/// Raw reads rather than a decode, deliberately: a *disabled* route's
/// projected request is a well-formed byte string that full validation may
/// refuse (a zero fee amount, for one), yet the Hot executor still hashes
/// exactly these bytes to seed that route's caller-authority coordinate, so
/// the builder must be able to derive the address from them regardless.
fn custody_market_and_context(request: &[u8]) -> Result<([u8; 32], [u8; 32]), BuilderError> {
    let base = if request.get(..8) == Some(DELEGATED_CUSTODY_REQUEST_MAGIC_V2.as_slice()) {
        DelegatedCustodyRequestLayoutV2::BASE
    } else {
        0
    };
    let read = |offset: usize| -> Result<[u8; 32], BuilderError> {
        request
            .get(base + offset..base + offset + 32)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(BuilderError::UnsupportedRoute)
    };
    Ok((
        read(CustodyRequestLayoutV1::MARKET)?,
        read(CustodyRequestLayoutV1::CONTEXT)?,
    ))
}

fn claims_context(request: &[u8]) -> Result<([u8; 32], [u8; 32]), BuilderError> {
    if request.get(..8) == Some(SPARSE_NATIVE_TRANSFER_MAGIC_V1.as_slice()) {
        let decoded =
            SparseNativeTransferV1::decode(request).map_err(|_| BuilderError::UnsupportedRoute)?;
        let input = decoded.input();
        Ok((input.market, input.request_id))
    } else if request.get(..8) == Some(PROTOCOL_POSITION_REQUEST_MAGIC_V2.as_slice()) {
        let decoded =
            ProtocolPositionRequestV2::decode(request).map_err(|_| BuilderError::UnsupportedRoute)?;
        Ok((decoded.market, decoded.position_owner))
    } else if request.get(..8) == Some(AFFINE_BATCH_PLAN_MAGIC_V2.as_slice()) {
        let decoded =
            AffineBatchPlanV2::decode(request).map_err(|_| BuilderError::UnsupportedRoute)?;
        Ok((decoded.market(), decoded.request_id()))
    } else if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
        let decoded =
            SignedDeltaPlanV3::decode(request).map_err(|_| BuilderError::UnsupportedRoute)?;
        Ok((decoded.market(), decoded.request_id()))
    } else if request.get(..8) == Some(CLAIMS_FOUNDING_REQUEST_MAGIC_V5.as_slice()) {
        let decoded =
            ClaimsFoundingRequestV5::decode(request).map_err(|_| BuilderError::UnsupportedRoute)?;
        Ok((decoded.market(), decoded.founding_intent_digest()))
    } else {
        // Rational representation/lifecycle request kinds are a named boundary:
        // add their contract crates and arms when a reproduced family needs them.
        Err(BuilderError::UnsupportedRoute)
    }
}
