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

use dclutch_claims::rational::{CallerRoleV2, REQUEST_MAGIC_V2, RepresentationRequestV2};
use dclutch_claims::{
    affine_batch_v2::{AFFINE_BATCH_PLAN_MAGIC_V2, AffineBatchPlanV2},
    founding_v5::{CLAIMS_FOUNDING_REQUEST_MAGIC_V5, ClaimsFoundingRequestV5},
    protocol_position_v2::{PROTOCOL_POSITION_REQUEST_MAGIC_V2, ProtocolPositionRequestV2},
    signed_delta_v3::{SIGNED_DELTA_PLAN_MAGIC_V3, SignedDeltaPlanV3},
    sparse_native_transfer_v1::{SPARSE_NATIVE_TRANSFER_MAGIC_V1, SparseNativeTransferV1},
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CustodyRequestLayoutV1, DELEGATED_CUSTODY_REQUEST_MAGIC_V2, DelegatedCustodyRequestLayoutV2,
    PROJECTED_CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_REQUEST_MAGIC_V1,
    ProjectedCustodyCallerSeedsV1, ProjectedCustodyRequestV1,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_vm::effect::v2::FixedRole;
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
    if invocation.resolved.role == FixedRole::Custody
        && (request.len() == PROJECTED_CUSTODY_REQUEST_BYTES_V1
            || request.get(..PROJECTED_CUSTODY_REQUEST_MAGIC_V1.len())
                == Some(PROJECTED_CUSTODY_REQUEST_MAGIC_V1.as_slice()))
    {
        let decoded = ProjectedCustodyRequestV1::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        if decoded.caller_program != trading_program.to_bytes() {
            return Err(BuilderError::UnsupportedRoute(line!()));
        }
        let seeds = ProjectedCustodyCallerSeedsV1::new(decoded, request_digest);
        let authority = Pubkey::find_program_address(&seeds.as_slices(), &trading_program).0;
        return Ok(Some(DerivedAuthorityV1 {
            coordinate: usize::from(invocation.resolved.fixed_account_start),
            authority,
            request_digest,
        }));
    }
    let (market, context) = match invocation.resolved.role {
        FixedRole::Core | FixedRole::Resolution => return Ok(None),
        FixedRole::Custody => custody_market_and_context(request)?,
        FixedRole::Claims => claims_context(request)?,
    };
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).map_err(|_| BuilderError::UnsupportedRoute(line!()))?,
        market,
        ExecutionRoleV1::Trading,
        context,
        request_digest,
    )
    .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
    let authority = Pubkey::find_program_address(&seeds.as_slices(), &trading_program).0;
    Ok(Some(DerivedAuthorityV1 {
        coordinate: usize::from(invocation.resolved.fixed_account_start),
        authority,
        request_digest,
    }))
}

#[cfg(test)]
mod tests {
    use dclutch_claims::rational::{
        ABSENT_REVISION, ASSET_BYTES_V3, AssetV2, CallerRoleV2, REQUEST_SELECTED_HEADER_BYTES_V3,
        RepresentationActionV2, RepresentationRequestHeaderV2, RepresentationRequestV2,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
    use dclutch_custody::{
        CompartmentV1, ProjectedCallerRoleV1, ProjectedCustodyOperationV1,
        ProjectedCustodyRequestV1,
    };
    use sha2::{Digest, Sha256};
    use solana_program::pubkey::Pubkey;

    use super::{
        BuilderError, PROJECTED_CUSTODY_REQUEST_BYTES_V1, claims_context, derive_authority,
    };
    use crate::registers::DerivedInvocationV1;
    use dclutch_vm::effect::v2::FixedRole;
    use dclutch_vm::effect::v3::{
        ResolvedInvocationV3, ResolvedReceiptDependenciesV3, RouteKindV3,
    };

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn request(trading_program: Pubkey) -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::AbortSourceAndClose,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: id(1),
            generation: 2,
            realm: id(3),
            product_record: id(4),
            product: id(5),
            source: id(6),
            release_set: id(7),
            projection_receipt_digest: id(8),
            parent_capability_root: id(9),
            context_digest: id(10),
            caller_program: trading_program.to_bytes(),
            payer: id(12),
            core_program: id(13),
            rent_program: id(14),
            refund_owner: id(15),
            rent_credit: id(16),
            hoard_vault: id(17),
            funding_source_vault: id(18),
            funding_source_context: id(19),
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: id(20),
            token_program: id(21),
            collateral_release: id(22),
            expiry_slot: 23,
            expected_revision: 3,
            resulting_revision: 4,
            amount: 25,
            state_rent_lamports: 26,
            vault_rent_lamports: 27,
            funding_source_replay_revision: 1,
            funding_source_state_rent_lamports: 29,
            funding_source_vault_rent_lamports: 30,
        }
    }

    fn invocation(bytes: Vec<u8>) -> DerivedInvocationV1 {
        DerivedInvocationV1 {
            route: 0,
            invocation: 0,
            resolved: ResolvedInvocationV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                item: None,
                fixed_account_start: 44,
                fixed_account_count: 11,
                item_account_start: 0,
                item_account_count: 0,
                item_account_stride: 0,
                repeated_item_count: 0,
                request_offset: 0,
                request_len: bytes.len(),
                borrowed_witness: None,
                receipt_dependencies: ResolvedReceiptDependenciesV3::empty(),
                receipt_dependency: None,
            },
            request: bytes,
        }
    }

    fn rational_request(caller_role: CallerRoleV2) -> Vec<u8> {
        let mut asset = [0_u8; ASSET_BYTES_V3];
        AssetV2 {
            shard_mint: id(20),
            actor_shard_account: id(21),
            structured_custody_account: id(22),
            claims_custody_owner: id(23),
            coefficient: 3,
            expected_shard_supply: 30,
            expected_actor_shards: 7,
            expected_structured_shards: 0,
        }
        .encode_into(&mut asset)
        .expect("asset");
        let request = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::Denominate,
                caller_role,
                release_set: id(1),
                market: id(2),
                graph_id: id(3),
                descriptor_id: id(4),
                parent_context: id(5),
                actor: id(6),
                receipt_mint: id(7),
                receipt_account: [0; 32],
                representation_authority: id(8),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 4,
                expected_claims_market_revision: 11,
                expected_actor_position_revision: 12,
                expected_custody_position_revision: 13,
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: 14,
                quantity: 2,
                denominator: 10,
                expected_receipt_supply: 0,
                outcome_count: 2,
                selected_outcome: 1,
                asset_count: 1,
            },
            &asset,
        )
        .expect("request");
        let mut bytes = vec![0_u8; REQUEST_SELECTED_HEADER_BYTES_V3 + ASSET_BYTES_V3];
        request.encode_into(&mut bytes).expect("encode");
        bytes
    }

    #[test]
    fn projected_custody_authority_is_exact_and_hostile() {
        let trading_program = Pubkey::new_from_array(id(11));
        let bytes = request(trading_program).encode().expect("request");
        let expected_digest: [u8; 32] = Sha256::digest(bytes).into();
        let derived = derive_authority(&invocation(bytes.to_vec()), id(7), trading_program)
            .expect("derive")
            .expect("authority");
        let expected_seeds = dclutch_custody::ProjectedCustodyCallerSeedsV1::new(
            request(trading_program),
            expected_digest,
        );
        assert_eq!(derived.coordinate, 44);
        assert_eq!(derived.request_digest, expected_digest);
        assert_eq!(
            derived.authority,
            Pubkey::find_program_address(&expected_seeds.as_slices(), &trading_program).0
        );

        let mut wrong_magic = bytes;
        wrong_magic[0] ^= 1;
        assert!(matches!(
            derive_authority(&invocation(wrong_magic.to_vec()), id(7), trading_program),
            Err(BuilderError::UnsupportedRoute(_))
        ));

        let mut wrong_reserved_field = bytes;
        wrong_reserved_field[13] = 1;
        assert!(matches!(
            derive_authority(
                &invocation(wrong_reserved_field.to_vec()),
                id(7),
                trading_program,
            ),
            Err(BuilderError::UnsupportedRoute(_))
        ));

        let mut changed_digest = bytes;
        changed_digest[688] ^= 1;
        let changed =
            derive_authority(&invocation(changed_digest.to_vec()), id(7), trading_program)
                .expect("changed request remains canonical")
                .expect("authority");
        assert_ne!(changed.request_digest, derived.request_digest);
        assert_ne!(changed.authority, derived.authority);

        assert!(matches!(
            derive_authority(
                &invocation(bytes.to_vec()),
                id(7),
                Pubkey::new_from_array(id(31)),
            ),
            Err(BuilderError::UnsupportedRoute(_))
        ));

        let truncated = bytes[..PROJECTED_CUSTODY_REQUEST_BYTES_V1 - 1].to_vec();
        assert!(matches!(
            derive_authority(&invocation(truncated), id(7), trading_program),
            Err(BuilderError::UnsupportedRoute(_))
        ));
    }

    #[test]
    fn rational_representation_context_is_decoded_and_trading_bound() {
        let bytes = rational_request(CallerRoleV2::Trading);
        assert_eq!(claims_context(&bytes), Ok((id(2), id(5))));

        let wrong_role = rational_request(CallerRoleV2::Core);
        assert!(matches!(
            claims_context(&wrong_role),
            Err(BuilderError::UnsupportedRoute(_))
        ));

        let mut noncanonical = bytes;
        // Still the last byte of the class's canonically zero reserved tail.
        noncanonical[REQUEST_SELECTED_HEADER_BYTES_V3 - 1] = 1;
        assert!(matches!(
            claims_context(&noncanonical),
            Err(BuilderError::UnsupportedRoute(_))
        ));
    }
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
            .ok_or(BuilderError::UnsupportedRoute(line!()))
    };
    Ok((
        read(CustodyRequestLayoutV1::MARKET)?,
        read(CustodyRequestLayoutV1::CONTEXT)?,
    ))
}

fn claims_context(request: &[u8]) -> Result<([u8; 32], [u8; 32]), BuilderError> {
    if request.get(..8) == Some(SPARSE_NATIVE_TRANSFER_MAGIC_V1.as_slice()) {
        let decoded = SparseNativeTransferV1::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        let input = decoded.input();
        Ok((input.market, input.request_id))
    } else if request.get(..8) == Some(PROTOCOL_POSITION_REQUEST_MAGIC_V2.as_slice()) {
        let decoded = ProtocolPositionRequestV2::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        Ok((decoded.market, decoded.position_owner))
    } else if request.get(..8) == Some(AFFINE_BATCH_PLAN_MAGIC_V2.as_slice()) {
        let decoded = AffineBatchPlanV2::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        Ok((decoded.market(), decoded.request_id()))
    } else if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
        let decoded = SignedDeltaPlanV3::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        Ok((decoded.market(), decoded.request_id()))
    } else if request.get(..8) == Some(CLAIMS_FOUNDING_REQUEST_MAGIC_V5.as_slice()) {
        let decoded = ClaimsFoundingRequestV5::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        Ok((decoded.market(), decoded.founding_intent_digest()))
    } else if request.get(..8) == Some(REQUEST_MAGIC_V2.as_slice()) {
        let decoded = RepresentationRequestV2::decode(request)
            .map_err(|_| BuilderError::UnsupportedRoute(line!()))?;
        let header = decoded.header();
        if header.caller_role != CallerRoleV2::Trading {
            return Err(BuilderError::UnsupportedRoute(line!()));
        }
        Ok((header.market, header.parent_context))
    } else {
        // Rational lifecycle remains a named boundary until a reproduced
        // family needs it. Representation is decoded through its semantic
        // owner above; no request offset is restated here.
        std::eprintln!(
            "UNKNOWN CLAIMS MAGIC len={} head={:?}",
            request.len(),
            request.get(..8)
        );
        Err(BuilderError::UnsupportedRoute(line!()))
    }
}
