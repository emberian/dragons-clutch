//! Canonical patchable coordinates of the generated Custody request wire.
//!
//! This narrow view delegates every numeric coordinate to the generated ABI.
//! Data-defined effect encoders may patch typed fields without copying the
//! generated layout or gaining authority over static tags and reserved bytes.

use crate::generated::*;

/// Canonical byte coordinates of patchable [`crate::CustodyRequestV1`] fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyRequestLayoutV1;

impl CustodyRequestLayoutV1 {
    /// Physical operation tag as `u8`.
    pub const OPERATION: usize = REQUEST_OPERATION_OFFSET;
    /// Source compartment tag as `u8`.
    pub const SOURCE_COMPARTMENT: usize = REQUEST_SOURCE_COMPARTMENT_OFFSET;
    /// Destination compartment tag as `u8`.
    pub const DESTINATION_COMPARTMENT: usize = REQUEST_DESTINATION_COMPARTMENT_OFFSET;
    /// Ordered effect coordinate as little-endian `u16`.
    pub const TRANSFER_INDEX: usize = REQUEST_TRANSFER_INDEX_OFFSET;
    /// Selected execution release set.
    pub const RELEASE_SET: usize = REQUEST_RELEASE_SET_OFFSET;
    /// Logical Market identity.
    pub const MARKET: usize = REQUEST_MARKET_OFFSET;
    /// Immutable Realm identity.
    pub const REALM: usize = REQUEST_REALM_OFFSET;
    /// Custody replay namespace.
    pub const CONTEXT: usize = REQUEST_CONTEXT_OFFSET;
    /// Exact caller program.
    pub const CALLER_PROGRAM: usize = REQUEST_CALLER_PROGRAM_OFFSET;
    /// Optional best-valid-submitted candidate identity.
    pub const CANDIDATE: usize = REQUEST_CANDIDATE_OFFSET;
    /// External source owner.
    pub const SOURCE_OWNER: usize = REQUEST_SOURCE_OWNER_OFFSET;
    /// External destination owner.
    pub const DESTINATION_OWNER: usize = REQUEST_DESTINATION_OWNER_OFFSET;
    /// Optional order identity.
    pub const ORDER: usize = REQUEST_ORDER_OFFSET;
    /// Complete parent-request digest.
    pub const PARENT_REQUEST_DIGEST: usize = REQUEST_PARENT_REQUEST_DIGEST_OFFSET;
    /// Source token account.
    pub const SOURCE: usize = REQUEST_SOURCE_OFFSET;
    /// Destination token account.
    pub const DESTINATION: usize = REQUEST_DESTINATION_OFFSET;
    /// Custody-owned source Vault namespace.
    pub const SOURCE_VAULT_CONTEXT: usize = REQUEST_SOURCE_VAULT_CONTEXT_OFFSET;
    /// Custody-owned destination Vault namespace.
    pub const DESTINATION_VAULT_CONTEXT: usize = REQUEST_DESTINATION_VAULT_CONTEXT_OFFSET;
    /// Realm-selected collateral Mint.
    pub const MINT: usize = REQUEST_MINT_OFFSET;
    /// Realm-selected Token program.
    pub const TOKEN_PROGRAM: usize = REQUEST_TOKEN_PROGRAM_OFFSET;
    /// Rent payer for create operations.
    pub const PAYER: usize = REQUEST_PAYER_OFFSET;
    /// Immutable rent-refund beneficiary.
    pub const RENT_REFUND: usize = REQUEST_RENT_REFUND_OFFSET;
    /// Optimistic replay pre-revision as little-endian `u64`.
    pub const EXPECTED_REVISION: usize = REQUEST_EXPECTED_REVISION_OFFSET;
    /// Exact next replay revision as little-endian `u64`.
    pub const RESULTING_REVISION: usize = REQUEST_RESULTING_REVISION_OFFSET;
    /// Caller-defined replay nonce as little-endian `u64`.
    pub const ORDER_NONCE: usize = REQUEST_ORDER_NONCE_OFFSET;
    /// Market generation as little-endian `u64`.
    pub const GENERATION: usize = REQUEST_GENERATION_OFFSET;
    /// Exact collateral amount as little-endian `u64`.
    pub const AMOUNT: usize = REQUEST_AMOUNT_OFFSET;
    /// Exact create/close rent lamports as little-endian `u64`.
    pub const RENT_LAMPORTS: usize = REQUEST_RENT_LAMPORTS_OFFSET;
    /// Page coordinate as little-endian `u32`.
    pub const PAGE_INDEX: usize = REQUEST_PAGE_INDEX_OFFSET;
    /// Execution coordinate as little-endian `u32`.
    pub const EXECUTION_INDEX: usize = REQUEST_EXECUTION_INDEX_OFFSET;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestV1, OperationV1};

    fn slice(bytes: &[u8], offset: usize, width: usize) -> &[u8] {
        bytes.get(offset..offset + width).expect("layout field")
    }

    #[test]
    fn public_layout_tracks_the_generated_request_encoder() {
        let request = CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::Settlement,
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            context: [4; 32],
            caller_program: [5; 32],
            semantic: ContextV1 {
                candidate: [6; 32],
                source_owner: [7; 32],
                destination_owner: [0; 32],
                order: [8; 32],
                parent_request_digest: [9; 32],
                order_nonce: 10,
                generation: 11,
                page_index: 12,
                execution_index: 13,
                transfer_index: 14,
            },
            source: [15; 32],
            destination: [16; 32],
            source_vault_context: [0; 32],
            destination_vault_context: [17; 32],
            mint: [18; 32],
            token_program: [19; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 20,
            resulting_revision: 21,
            amount: 22,
            rent_lamports: 0,
        };
        let bytes = request.to_bytes().expect("request");
        assert_eq!(slice(&bytes, CustodyRequestLayoutV1::OPERATION, 1), &[2]);
        assert_eq!(
            slice(&bytes, CustodyRequestLayoutV1::SOURCE_COMPARTMENT, 1),
            &[1]
        );
        assert_eq!(
            slice(&bytes, CustodyRequestLayoutV1::DESTINATION_COMPARTMENT, 1),
            &[2]
        );
        assert_eq!(
            slice(&bytes, CustodyRequestLayoutV1::TRANSFER_INDEX, 2),
            &14_u16.to_le_bytes()
        );
        for (offset, expected) in [
            (CustodyRequestLayoutV1::RELEASE_SET, request.release_set),
            (CustodyRequestLayoutV1::MARKET, request.market),
            (CustodyRequestLayoutV1::REALM, request.realm),
            (CustodyRequestLayoutV1::CONTEXT, request.context),
            (
                CustodyRequestLayoutV1::CALLER_PROGRAM,
                request.caller_program,
            ),
            (
                CustodyRequestLayoutV1::CANDIDATE,
                request.semantic.candidate,
            ),
            (
                CustodyRequestLayoutV1::SOURCE_OWNER,
                request.semantic.source_owner,
            ),
            (
                CustodyRequestLayoutV1::DESTINATION_OWNER,
                request.semantic.destination_owner,
            ),
            (CustodyRequestLayoutV1::ORDER, request.semantic.order),
            (
                CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
                request.semantic.parent_request_digest,
            ),
            (CustodyRequestLayoutV1::SOURCE, request.source),
            (CustodyRequestLayoutV1::DESTINATION, request.destination),
            (
                CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT,
                request.source_vault_context,
            ),
            (
                CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
                request.destination_vault_context,
            ),
            (CustodyRequestLayoutV1::MINT, request.mint),
            (CustodyRequestLayoutV1::TOKEN_PROGRAM, request.token_program),
            (CustodyRequestLayoutV1::PAYER, request.payer),
            (CustodyRequestLayoutV1::RENT_REFUND, request.rent_refund),
        ] {
            assert_eq!(slice(&bytes, offset, 32), expected.as_slice());
        }
        for (offset, expected) in [
            (
                CustodyRequestLayoutV1::EXPECTED_REVISION,
                request.expected_revision,
            ),
            (
                CustodyRequestLayoutV1::RESULTING_REVISION,
                request.resulting_revision,
            ),
            (
                CustodyRequestLayoutV1::ORDER_NONCE,
                request.semantic.order_nonce,
            ),
            (
                CustodyRequestLayoutV1::GENERATION,
                request.semantic.generation,
            ),
            (CustodyRequestLayoutV1::AMOUNT, request.amount),
            (CustodyRequestLayoutV1::RENT_LAMPORTS, request.rent_lamports),
        ] {
            assert_eq!(slice(&bytes, offset, 8), expected.to_le_bytes().as_slice());
        }
        assert_eq!(
            slice(&bytes, CustodyRequestLayoutV1::PAGE_INDEX, 4),
            &12_u32.to_le_bytes()
        );
        assert_eq!(
            slice(&bytes, CustodyRequestLayoutV1::EXECUTION_INDEX, 4),
            &13_u32.to_le_bytes()
        );
        assert_eq!(CustodyRequestV1::decode(&bytes), Ok(request));
    }
}
