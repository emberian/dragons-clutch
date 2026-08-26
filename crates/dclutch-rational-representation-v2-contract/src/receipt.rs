//! State-last completion and normalized composition receipt.

use dclutch_claims_svm::affine_batch_v2::{
    AffineBatchPlanV2, AffineBatchReceiptV2, DeltaDirectionV2,
};
use dclutch_claims_svm::lbv2_terminal_v2::{
    Lbv2TerminalRedeemReceiptV2, Lbv2TerminalRedeemRequestV2,
};
use dclutch_claims_svm::signed_delta_v3::{
    DeltaDirectionV3, SignedDeltaPlanV3, SignedDeltaReceiptV3,
};
use dclutch_custody_contract::{
    CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1, CustodyReceiptV1, CustodyRequestV1,
};

use crate::request::{CallerRoleV2, RepresentationActionV2, RepresentationRequestV2};
use crate::{
    ABSENT_REVISION, Error, Result, array_at, byte_at,
    generated::*,
    is_zero,
    plan::{AffineBatchContextV2, PreparedRepresentationV2},
    put, put_byte, require_nonzero, require_zero, subslice, u16_at, u32_at, u64_at,
};

/// Bytes in one post-Token asset observation: Mint supply, actor balance, and
/// Structured custody balance as three little-endian `u64` values.
pub const POST_ASSET_OBSERVATION_BYTES_V2: usize = 24;

// The normalized physical receipt width is unchanged. This field now commits
// the canonical affine packet rather than the retired scalar Claims packet.
const RECEIPT_AFFINE_PACKET_DIGEST_OFFSET: usize = RECEIPT_CLAIMS_PLAN_DIGEST_OFFSET;

/// Exact evidence observed after all child effects and before replay state is
/// committed. Every digest is computed by the physical adapter over exact
/// bytes; this no-crypto contract checks joins and shapes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionEvidenceV2<'a> {
    /// SHA-256 of the complete canonical representation request.
    pub request_digest: [u8; 32],
    /// Registry-authenticated current Claims program which owns this adapter.
    pub representation_program: [u8; 32],
    /// Same current Claims program selected for the affine batch.
    pub claims_program: [u8; 32],
    /// SHA-256 of exact affine packet bytes, zero for Structured actions.
    pub affine_packet_digest: [u8; 32],
    /// Exact affine packet, absent for Structured actions.
    pub affine_packet: Option<AffineBatchPlanV2<'a>>,
    /// Finalized Product/LiabilityBasis identities authenticated before CPI.
    pub affine_context: Option<AffineBatchContextV2>,
    /// Exact affine-batch return data, absent for Structured actions.
    pub affine_receipt: Option<AffineBatchReceiptV2>,
    /// Exact typed terminal Claims request, present only for terminal redemption.
    pub terminal_request: Option<&'a Lbv2TerminalRedeemRequestV2>,
    /// SHA-256 of the exact terminal Claims request bytes.
    pub terminal_request_digest: [u8; 32],
    /// Exact typed terminal Claims receipt, present only for terminal redemption.
    pub terminal_receipt: Option<&'a Lbv2TerminalRedeemReceiptV2>,
    /// SHA-256 of the ordered exact Token effect transcript.
    pub token_effect_digest: [u8; 32],
    /// Exact post receipt Mint supply.
    pub post_receipt_supply: u64,
    /// Repeated `(shard_supply, actor_shards, structured_shards)` observations.
    pub post_asset_observations: &'a [u8],
    /// Custody request for positive terminal payout only.
    pub custody_request: Option<&'a CustodyRequestV1>,
    /// SHA-256 of Custody request bytes, zero when inactive.
    pub custody_request_digest: [u8; 32],
    /// Custody return receipt for positive terminal payout only.
    pub custody_receipt: Option<&'a CustodyReceiptV1>,
    /// SHA-256 of exact Custody receipt bytes, zero when inactive.
    pub custody_receipt_digest: [u8; 32],
    /// SHA-256 of exact post Custody replay bytes, zero when inactive.
    pub custody_replay_digest: [u8; 32],
    /// SHA-256 of all exact post Claims, Token, and replay resources.
    pub post_resource_digest: [u8; 32],
}

/// Fixed normalized receipt returned only after every child postcondition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationReceiptV2 {
    action: RepresentationActionV2,
    caller_role: CallerRoleV2,
    release_set: [u8; 32],
    market: [u8; 32],
    graph_id: [u8; 32],
    descriptor_id: [u8; 32],
    parent_context: [u8; 32],
    request_digest: [u8; 32],
    actor: [u8; 32],
    representation_program: [u8; 32],
    claims_program: [u8; 32],
    token_program: [u8; 32],
    affine_packet_digest: [u8; 32],
    claims_resource_digest: [u8; 32],
    token_effect_digest: [u8; 32],
    custody_request_digest: [u8; 32],
    custody_receipt_digest: [u8; 32],
    post_resource_digest: [u8; 32],
    pre_representation_revision: u64,
    post_representation_revision: u64,
    post_claims_market_revision: u64,
    post_actor_position_revision: u64,
    post_custody_position_revision: u64,
    post_receipt_supply: u64,
    payout: u64,
    outcome_count: u32,
}

impl RepresentationReceiptV2 {
    /// Hostile-decode one exact normalized receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != RECEIPT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, RECEIPT_MAGIC_OFFSET)? != RECEIPT_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, RECEIPT_VERSION_OFFSET)? != PHYSICAL_ABI_VERSION_V2 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, RECEIPT_RESERVED_HEADER_OFFSET, 4)?;
        require_zero(input, RECEIPT_RESERVED_TAIL_OFFSET, 4)?;
        let value = Self {
            action: decode_action(byte_at(input, RECEIPT_ACTION_OFFSET)?)?,
            caller_role: decode_role(byte_at(input, RECEIPT_CALLER_ROLE_OFFSET)?)?,
            release_set: require_nonzero(array_at(input, RECEIPT_RELEASE_SET_OFFSET)?)?,
            market: require_nonzero(array_at(input, RECEIPT_MARKET_OFFSET)?)?,
            graph_id: require_nonzero(array_at(input, RECEIPT_GRAPH_ID_OFFSET)?)?,
            descriptor_id: require_nonzero(array_at(input, RECEIPT_DESCRIPTOR_ID_OFFSET)?)?,
            parent_context: require_nonzero(array_at(input, RECEIPT_PARENT_CONTEXT_OFFSET)?)?,
            request_digest: require_nonzero(array_at(input, RECEIPT_REQUEST_DIGEST_OFFSET)?)?,
            actor: require_nonzero(array_at(input, RECEIPT_ACTOR_OFFSET)?)?,
            representation_program: require_nonzero(array_at(
                input,
                RECEIPT_REPRESENTATION_PROGRAM_OFFSET,
            )?)?,
            claims_program: require_nonzero(array_at(input, RECEIPT_CLAIMS_PROGRAM_OFFSET)?)?,
            token_program: require_nonzero(array_at(input, RECEIPT_TOKEN_PROGRAM_OFFSET)?)?,
            affine_packet_digest: array_at(input, RECEIPT_AFFINE_PACKET_DIGEST_OFFSET)?,
            claims_resource_digest: array_at(input, RECEIPT_CLAIMS_RESOURCE_DIGEST_OFFSET)?,
            token_effect_digest: require_nonzero(array_at(
                input,
                RECEIPT_TOKEN_EFFECT_DIGEST_OFFSET,
            )?)?,
            custody_request_digest: array_at(input, RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET)?,
            custody_receipt_digest: array_at(input, RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET)?,
            post_resource_digest: require_nonzero(array_at(
                input,
                RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
            )?)?,
            pre_representation_revision: u64_at(input, RECEIPT_PRE_REPRESENTATION_REVISION_OFFSET)?,
            post_representation_revision: u64_at(
                input,
                RECEIPT_POST_REPRESENTATION_REVISION_OFFSET,
            )?,
            post_claims_market_revision: u64_at(input, RECEIPT_POST_CLAIMS_MARKET_REVISION_OFFSET)?,
            post_actor_position_revision: u64_at(
                input,
                RECEIPT_POST_ACTOR_POSITION_REVISION_OFFSET,
            )?,
            post_custody_position_revision: u64_at(
                input,
                RECEIPT_POST_CUSTODY_POSITION_REVISION_OFFSET,
            )?,
            post_receipt_supply: u64_at(input, RECEIPT_POST_RECEIPT_SUPPLY_OFFSET)?,
            payout: u64_at(input, RECEIPT_PAYOUT_OFFSET)?,
            outcome_count: u32_at(input, RECEIPT_OUTCOME_COUNT_OFFSET)?,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode one exact normalized receipt.
    pub fn to_bytes(self) -> Result<[u8; RECEIPT_BYTES_V2]> {
        self.validate_shape()?;
        let mut output = [0_u8; RECEIPT_BYTES_V2];
        put(&mut output, RECEIPT_MAGIC_OFFSET, &RECEIPT_MAGIC_V2)?;
        put(
            &mut output,
            RECEIPT_VERSION_OFFSET,
            &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
        )?;
        put_byte(&mut output, RECEIPT_ACTION_OFFSET, self.action as u8)?;
        put_byte(
            &mut output,
            RECEIPT_CALLER_ROLE_OFFSET,
            self.caller_role as u8,
        )?;
        for (offset, value) in [
            (RECEIPT_RELEASE_SET_OFFSET, self.release_set),
            (RECEIPT_MARKET_OFFSET, self.market),
            (RECEIPT_GRAPH_ID_OFFSET, self.graph_id),
            (RECEIPT_DESCRIPTOR_ID_OFFSET, self.descriptor_id),
            (RECEIPT_PARENT_CONTEXT_OFFSET, self.parent_context),
            (RECEIPT_REQUEST_DIGEST_OFFSET, self.request_digest),
            (RECEIPT_ACTOR_OFFSET, self.actor),
            (
                RECEIPT_REPRESENTATION_PROGRAM_OFFSET,
                self.representation_program,
            ),
            (RECEIPT_CLAIMS_PROGRAM_OFFSET, self.claims_program),
            (RECEIPT_TOKEN_PROGRAM_OFFSET, self.token_program),
            (
                RECEIPT_AFFINE_PACKET_DIGEST_OFFSET,
                self.affine_packet_digest,
            ),
            (
                RECEIPT_CLAIMS_RESOURCE_DIGEST_OFFSET,
                self.claims_resource_digest,
            ),
            (RECEIPT_TOKEN_EFFECT_DIGEST_OFFSET, self.token_effect_digest),
            (
                RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET,
                self.custody_request_digest,
            ),
            (
                RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET,
                self.custody_receipt_digest,
            ),
            (
                RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
                self.post_resource_digest,
            ),
        ] {
            put(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (
                RECEIPT_PRE_REPRESENTATION_REVISION_OFFSET,
                self.pre_representation_revision,
            ),
            (
                RECEIPT_POST_REPRESENTATION_REVISION_OFFSET,
                self.post_representation_revision,
            ),
            (
                RECEIPT_POST_CLAIMS_MARKET_REVISION_OFFSET,
                self.post_claims_market_revision,
            ),
            (
                RECEIPT_POST_ACTOR_POSITION_REVISION_OFFSET,
                self.post_actor_position_revision,
            ),
            (
                RECEIPT_POST_CUSTODY_POSITION_REVISION_OFFSET,
                self.post_custody_position_revision,
            ),
            (RECEIPT_POST_RECEIPT_SUPPLY_OFFSET, self.post_receipt_supply),
            (RECEIPT_PAYOUT_OFFSET, self.payout),
        ] {
            put(&mut output, offset, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            RECEIPT_OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// SHA-256 of the exact parent request.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }

    /// SHA-256 of the exact canonical affine or typed terminal Claims request.
    pub const fn affine_packet_digest(self) -> [u8; 32] {
        self.affine_packet_digest
    }

    /// Resulting canonical Claims aggregate revision, or the absent sentinel.
    pub const fn post_claims_market_revision(self) -> u64 {
        self.post_claims_market_revision
    }

    /// Resulting custody ProtocolPosition revision, or the absent sentinel.
    pub const fn post_custody_position_revision(self) -> u64 {
        self.post_custody_position_revision
    }

    /// Exact payout derived by canonical Claims economics.
    pub const fn payout(self) -> u64 {
        self.payout
    }

    /// Resulting representation replay revision.
    pub const fn post_representation_revision(self) -> u64 {
        self.post_representation_revision
    }

    /// SHA-256 of all exact post resources.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }

    /// Require this receipt to bind one exact request and digest.
    pub fn verify_for(
        self,
        request: RepresentationRequestV2<'_>,
        request_digest: [u8; 32],
    ) -> Result<()> {
        let header = request.header();
        if self.action != header.action
            || self.caller_role != header.caller_role
            || self.release_set != header.release_set
            || self.market != header.market
            || self.graph_id != header.graph_id
            || self.descriptor_id != header.descriptor_id
            || self.parent_context != header.parent_context
            || self.actor != header.actor
            || self.request_digest != request_digest
            || self.pre_representation_revision != header.expected_representation_revision
            || self.outcome_count != header.outcome_count
        {
            return Err(Error::ReceiptMismatch);
        }
        Ok(())
    }

    fn validate_shape(self) -> Result<()> {
        for value in [
            self.release_set,
            self.market,
            self.graph_id,
            self.descriptor_id,
            self.parent_context,
            self.request_digest,
            self.actor,
            self.representation_program,
            self.claims_program,
            self.token_program,
            self.token_effect_digest,
            self.post_resource_digest,
        ] {
            require_nonzero(value)?;
        }
        if self.representation_program != self.claims_program
            || self.outcome_count == 0
            || self.post_representation_revision
                != self
                    .pre_representation_revision
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::ReceiptMismatch);
        }
        let affine = matches!(
            self.action,
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
        );
        let claims = affine || self.action == RepresentationActionV2::RedeemTerminal;
        if claims
            != (!is_zero(self.affine_packet_digest)
                && !is_zero(self.claims_resource_digest)
                && self.post_claims_market_revision != ABSENT_REVISION)
        {
            return Err(Error::ReceiptMismatch);
        }
        if !affine && self.action != RepresentationActionV2::RedeemTerminal && self.payout != 0 {
            return Err(Error::InvalidActionShape);
        }
        let custody =
            !is_zero(self.custody_request_digest) || !is_zero(self.custody_receipt_digest);
        if custody != (self.action == RepresentationActionV2::RedeemTerminal && self.payout > 0)
            || is_zero(self.custody_request_digest) != is_zero(self.custody_receipt_digest)
        {
            return Err(Error::ReceiptMismatch);
        }
        Ok(())
    }
}

/// Validate all child postconditions, then construct the only receipt which a
/// physical adapter may persist alongside replay revision.
pub fn finalize(
    prepared: PreparedRepresentationV2<'_>,
    evidence: CompletionEvidenceV2<'_>,
) -> Result<RepresentationReceiptV2> {
    let request = prepared.request();
    let header = request.header();
    for digest in [
        evidence.request_digest,
        evidence.representation_program,
        evidence.claims_program,
        evidence.token_effect_digest,
        evidence.post_resource_digest,
    ] {
        require_nonzero(digest)?;
    }
    if evidence.representation_program != evidence.claims_program {
        return Err(Error::ReceiptMismatch);
    }
    let post_representation_revision = header
        .expected_representation_revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let expected_receipt_supply = match header.action {
        RepresentationActionV2::IssueStructured => header
            .expected_receipt_supply
            .checked_add(header.quantity)
            .ok_or(Error::ArithmeticOverflow)?,
        RepresentationActionV2::UnwrapStructured => header
            .expected_receipt_supply
            .checked_sub(header.quantity)
            .ok_or(Error::InsufficientBalance)?,
        RepresentationActionV2::Denominate
        | RepresentationActionV2::Reconstitute
        | RepresentationActionV2::RedeemTerminal => header.expected_receipt_supply,
    };
    if evidence.post_receipt_supply != expected_receipt_supply {
        return Err(Error::TokenMismatch);
    }
    validate_post_assets(prepared, evidence.post_asset_observations)?;
    let claims = validate_claims(prepared, evidence)?;
    validate_custody(prepared, evidence, claims.payout)?;
    let receipt = RepresentationReceiptV2 {
        action: header.action,
        caller_role: header.caller_role,
        release_set: header.release_set,
        market: header.market,
        graph_id: header.graph_id,
        descriptor_id: header.descriptor_id,
        parent_context: header.parent_context,
        request_digest: evidence.request_digest,
        actor: header.actor,
        representation_program: evidence.representation_program,
        claims_program: evidence.claims_program,
        token_program: header.token_program,
        affine_packet_digest: claims.effect_digest,
        claims_resource_digest: claims.resource_digest,
        token_effect_digest: evidence.token_effect_digest,
        custody_request_digest: evidence.custody_request_digest,
        custody_receipt_digest: evidence.custody_receipt_digest,
        post_resource_digest: evidence.post_resource_digest,
        pre_representation_revision: header.expected_representation_revision,
        post_representation_revision,
        post_claims_market_revision: claims.market_revision,
        post_actor_position_revision: claims.actor_position_revision,
        post_custody_position_revision: claims.custody_position_revision,
        post_receipt_supply: evidence.post_receipt_supply,
        payout: claims.payout,
        outcome_count: header.outcome_count,
    };
    receipt.validate_shape()?;
    Ok(receipt)
}

fn validate_post_assets(prepared: PreparedRepresentationV2<'_>, observations: &[u8]) -> Result<()> {
    let request = prepared.request();
    let header = request.header();
    let expected_bytes = usize::try_from(header.asset_count)
        .map_err(|_| Error::InvalidWidth)?
        .checked_mul(POST_ASSET_OBSERVATION_BYTES_V2)
        .ok_or(Error::InvalidLength)?;
    if observations.len() != expected_bytes {
        return Err(Error::InvalidLength);
    }
    let mut index = 0_u32;
    while index < header.asset_count {
        let asset = request.asset(index)?;
        let offset = usize::try_from(index)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(POST_ASSET_OBSERVATION_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        let post_supply = u64::from_le_bytes(
            subslice(observations, offset, 8)?
                .try_into()
                .map_err(|_| Error::InvalidLength)?,
        );
        let post_actor = u64::from_le_bytes(
            subslice(observations, offset + 8, 8)?
                .try_into()
                .map_err(|_| Error::InvalidLength)?,
        );
        let post_structured = u64::from_le_bytes(
            subslice(observations, offset + 16, 8)?
                .try_into()
                .map_err(|_| Error::InvalidLength)?,
        );
        let selected_amount = header
            .denominator
            .checked_mul(header.quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let structured_amount = asset
            .coefficient
            .checked_mul(header.quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let expected = match header.action {
            RepresentationActionV2::Denominate => (
                asset
                    .expected_shard_supply
                    .checked_add(selected_amount)
                    .ok_or(Error::ArithmeticOverflow)?,
                asset
                    .expected_actor_shards
                    .checked_add(selected_amount)
                    .ok_or(Error::ArithmeticOverflow)?,
                asset.expected_structured_shards,
            ),
            RepresentationActionV2::Reconstitute | RepresentationActionV2::RedeemTerminal => (
                asset
                    .expected_shard_supply
                    .checked_sub(selected_amount)
                    .ok_or(Error::InsufficientBalance)?,
                asset
                    .expected_actor_shards
                    .checked_sub(selected_amount)
                    .ok_or(Error::InsufficientBalance)?,
                asset.expected_structured_shards,
            ),
            RepresentationActionV2::IssueStructured => (
                asset.expected_shard_supply,
                asset
                    .expected_actor_shards
                    .checked_sub(structured_amount)
                    .ok_or(Error::InsufficientBalance)?,
                asset
                    .expected_structured_shards
                    .checked_add(structured_amount)
                    .ok_or(Error::ArithmeticOverflow)?,
            ),
            RepresentationActionV2::UnwrapStructured => (
                asset.expected_shard_supply,
                asset
                    .expected_actor_shards
                    .checked_add(structured_amount)
                    .ok_or(Error::ArithmeticOverflow)?,
                asset
                    .expected_structured_shards
                    .checked_sub(structured_amount)
                    .ok_or(Error::InsufficientBalance)?,
            ),
        };
        if (post_supply, post_actor, post_structured) != expected {
            return Err(Error::TokenMismatch);
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ClaimsCompletionV2 {
    effect_digest: [u8; 32],
    resource_digest: [u8; 32],
    market_revision: u64,
    actor_position_revision: u64,
    custody_position_revision: u64,
    payout: u64,
}

fn validate_claims(
    prepared: PreparedRepresentationV2<'_>,
    evidence: CompletionEvidenceV2<'_>,
) -> Result<ClaimsCompletionV2> {
    let header = prepared.request().header();
    if matches!(
        header.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    ) {
        if evidence.affine_packet.is_some()
            || evidence.affine_context.is_some()
            || evidence.affine_receipt.is_some()
            || !is_zero(evidence.affine_packet_digest)
            || evidence.terminal_request.is_some()
            || evidence.terminal_receipt.is_some()
            || !is_zero(evidence.terminal_request_digest)
        {
            return Err(Error::ClaimsMismatch);
        }
        return Ok(ClaimsCompletionV2 {
            effect_digest: [0; 32],
            resource_digest: [0; 32],
            market_revision: ABSENT_REVISION,
            actor_position_revision: ABSENT_REVISION,
            custody_position_revision: ABSENT_REVISION,
            payout: 0,
        });
    }
    if header.action == RepresentationActionV2::RedeemTerminal {
        if evidence.affine_packet.is_some()
            || evidence.affine_context.is_some()
            || evidence.affine_receipt.is_some()
            || !is_zero(evidence.affine_packet_digest)
        {
            return Err(Error::ClaimsMismatch);
        }
        let request = *evidence.terminal_request.ok_or(Error::ClaimsMismatch)?;
        let receipt = *evidence.terminal_receipt.ok_or(Error::ClaimsMismatch)?;
        require_nonzero(evidence.terminal_request_digest)?;
        receipt
            .verify_for(request, evidence.terminal_request_digest)
            .map_err(|_| Error::ClaimsMismatch)?;
        let asset = prepared.request().asset(0)?;
        if request.release_set() != header.release_set
            || request.market() != header.market
            || request.owner() != asset.claims_custody_owner
            || request.claim_index() != header.selected_outcome
            || request.pre_market_revision() != header.expected_claims_market_revision
            || request.pre_position_revision() != header.expected_custody_position_revision
            || request.debit_quantity() != header.quantity
            || request.claims_program() != evidence.claims_program
            || request.custody_request_digest() != evidence.custody_request_digest
            || receipt.custody_receipt_digest() != evidence.custody_receipt_digest
            || receipt.custody_replay_digest() != evidence.custody_replay_digest
            || (request.evaluated_payout() == 0
                && header.expected_custody_replay_revision != ABSENT_REVISION)
            || (request.evaluated_payout() != 0
                && request.pre_custody_revision() != header.expected_custody_replay_revision)
        {
            return Err(Error::ClaimsMismatch);
        }
        return Ok(ClaimsCompletionV2 {
            effect_digest: evidence.terminal_request_digest,
            resource_digest: receipt.post_resource_digest(),
            market_revision: request.post_market_revision(),
            actor_position_revision: ABSENT_REVISION,
            custody_position_revision: request.post_position_revision(),
            payout: request.evaluated_payout(),
        });
    }
    if evidence.terminal_request.is_some()
        || evidence.terminal_receipt.is_some()
        || !is_zero(evidence.terminal_request_digest)
    {
        return Err(Error::ClaimsMismatch);
    }
    require_nonzero(evidence.affine_packet_digest)?;
    let packet = evidence.affine_packet.ok_or(Error::ClaimsMismatch)?;
    let context = evidence.affine_context.ok_or(Error::ClaimsMismatch)?;
    let receipt = evidence.affine_receipt.ok_or(Error::ClaimsMismatch)?;
    receipt
        .validate_plan(packet)
        .map_err(|_| Error::ClaimsMismatch)?;
    let expected_role = match header.caller_role {
        CallerRoleV2::Core => dclutch_claims_svm::CallerRole::Core,
        CallerRoleV2::Trading => dclutch_claims_svm::CallerRole::Trading,
    };
    if packet.caller_role() != expected_role
        || packet.release_set() != header.release_set
        || packet.market() != header.market
        || packet.request_id() != evidence.request_digest
        || packet.product_record_digest() != context.product_record_digest
        || packet.semantic_basis_id() != context.semantic_basis_id
        || packet.linked_basis_record_digest() != context.linked_basis_record_digest
        || packet.expected_market_revision() != header.expected_claims_market_revision
        || packet.outcome_count() != header.outcome_count
        || packet.position_count() != 2
        || packet.row_count() != 1
        || receipt.packet_digest() != evidence.affine_packet_digest
        || receipt.claims_program() != evidence.claims_program
        || receipt.pre_market_revision() != header.expected_claims_market_revision
        || receipt.post_market_revision()
            != header
                .expected_claims_market_revision
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::ClaimsMismatch);
    }
    let asset = prepared.request().asset(0)?;
    let actor = packet.position(0).map_err(|_| Error::ClaimsMismatch)?;
    let custody = packet.position(1).map_err(|_| Error::ClaimsMismatch)?;
    if actor.owner() != header.actor
        || actor.expected_revision() != header.expected_actor_position_revision
        || custody.owner() != asset.claims_custody_owner
        || custody.expected_revision() != header.expected_custody_position_revision
    {
        return Err(Error::ClaimsMismatch);
    }
    let row = packet.row(0).map_err(|_| Error::ClaimsMismatch)?;
    let (source_index, destination_index) = match header.action {
        RepresentationActionV2::Denominate => (0, 1),
        RepresentationActionV2::Reconstitute => (1, 0),
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured => {
            return Err(Error::ClaimsMismatch);
        }
        RepresentationActionV2::RedeemTerminal => return Err(Error::InvalidActionShape),
    };
    if !row.source_present()
        || !row.destination_present()
        || row.outcome() != header.selected_outcome
        || row.source_position_index() != source_index
        || row.destination_position_index() != destination_index
        || row.aggregate_delta().direction() != DeltaDirectionV2::Neutral
        || row.aggregate_delta().magnitude() != 0
        || row.source_delta().direction() != DeltaDirectionV2::Debit
        || row.source_delta().magnitude() != header.quantity
        || row.destination_delta().direction() != DeltaDirectionV2::Credit
        || row.destination_delta().magnitude() != header.quantity
    {
        return Err(Error::ClaimsMismatch);
    }
    Ok(ClaimsCompletionV2 {
        effect_digest: evidence.affine_packet_digest,
        resource_digest: receipt.post_resource_digest(),
        market_revision: receipt.post_market_revision(),
        actor_position_revision: header
            .expected_actor_position_revision
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?,
        custody_position_revision: header
            .expected_custody_position_revision
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?,
        payout: 0,
    })
}

fn validate_custody(
    prepared: PreparedRepresentationV2<'_>,
    evidence: CompletionEvidenceV2<'_>,
    payout: u64,
) -> Result<()> {
    if payout == 0 {
        if evidence.custody_request.is_some()
            || evidence.custody_receipt.is_some()
            || !is_zero(evidence.custody_request_digest)
            || !is_zero(evidence.custody_receipt_digest)
            || !is_zero(evidence.custody_replay_digest)
        {
            return Err(Error::CustodyMismatch);
        }
        return Ok(());
    }
    let request = evidence.custody_request.ok_or(Error::CustodyMismatch)?;
    let receipt = evidence.custody_receipt.ok_or(Error::CustodyMismatch)?;
    for digest in [
        evidence.custody_request_digest,
        evidence.custody_receipt_digest,
        evidence.custody_replay_digest,
    ] {
        require_nonzero(digest)?;
    }
    let header = prepared.request().header();
    if request.amount != payout
        || request.caller_role != CustodyCallerRoleV1::Claims
        || request.source_compartment != CompartmentV1::HoardPrincipal
        || request.destination_compartment != CompartmentV1::External
        || request.release_set != header.release_set
        || request.market != header.market
        || request.realm != header.realm
        || request.semantic.destination_owner != header.actor
        || request.destination != header.collateral_recipient
    {
        return Err(Error::CustodyMismatch);
    }
    receipt
        .verify_for(
            *request,
            evidence.custody_request_digest,
            evidence.custody_replay_digest,
        )
        .map_err(|_| Error::CustodyMismatch)
}

/// Exact family-neutral terminal Claims evidence produced by the ProductBasisV3
/// planner and canonical SignedDeltaV3 executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaTerminalEvidenceV3<'a> {
    /// SHA-256 of the complete enclosing representation request.
    pub request_digest: [u8; 32],
    /// Current Registry-authenticated Claims program which wrote state.
    pub claims_program: [u8; 32],
    /// SHA-256 of the exact canonical SignedDeltaV3 packet.
    pub packet_digest: [u8; 32],
    /// Exact runtime-width terminal debit packet.
    pub packet: SignedDeltaPlanV3<'a>,
    /// Exact canonical receipt returned after Claims state committed.
    pub receipt: SignedDeltaReceiptV3,
}

/// Checked terminal Claims postcondition projected for representation replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaTerminalCompletionV3 {
    /// SHA-256 of the exact canonical terminal packet.
    pub effect_digest: [u8; 32],
    /// SHA-256 of the post aggregate followed by the post Position.
    pub resource_digest: [u8; 32],
    /// Canonical aggregate post-revision.
    pub market_revision: u64,
    /// Canonical custody Position post-revision.
    pub custody_position_revision: u64,
}

/// Validate the sole ProductBasisV3 terminal Claims shape.
///
/// The packet contains one already-netted debit at the selected native claim.
/// Product evaluation and collateral payout remain outside this no-crypto
/// receipt boundary; those facts are authenticated by the Product V3 reader
/// and typed Custody request respectively.
pub fn validate_signed_delta_terminal_v3(
    request: RepresentationRequestV2<'_>,
    evidence: SignedDeltaTerminalEvidenceV3<'_>,
) -> Result<SignedDeltaTerminalCompletionV3> {
    let header = request.header();
    if header.action != RepresentationActionV2::RedeemTerminal
        || is_zero(evidence.request_digest)
        || is_zero(evidence.claims_program)
        || is_zero(evidence.packet_digest)
    {
        return Err(Error::ClaimsMismatch);
    }
    evidence
        .receipt
        .validate_plan(evidence.packet)
        .map_err(|_| Error::ClaimsMismatch)?;
    let expected_role = match header.caller_role {
        CallerRoleV2::Core => dclutch_claims_svm::CallerRole::Core,
        CallerRoleV2::Trading => dclutch_claims_svm::CallerRole::Trading,
    };
    let asset = request.asset(0)?;
    let position = evidence
        .packet
        .position(0)
        .map_err(|_| Error::ClaimsMismatch)?;
    let aggregate = evidence
        .packet
        .aggregate_delta(header.selected_outcome)
        .map_err(|_| Error::ClaimsMismatch)?;
    let row = evidence
        .packet
        .position_delta(0)
        .map_err(|_| Error::ClaimsMismatch)?;
    let post_market_revision = header
        .expected_claims_market_revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let post_position_revision = header
        .expected_custody_position_revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if evidence.packet.caller_role() != expected_role
        || evidence.packet.release_set() != header.release_set
        || evidence.packet.market() != header.market
        || evidence.packet.request_id() != evidence.request_digest
        || evidence.packet.expected_market_revision() != header.expected_claims_market_revision
        || evidence.packet.claim_count() != header.outcome_count
        || evidence.packet.position_count() != 1
        || evidence.packet.position_delta_count() != 1
        || position.owner() != asset.claims_custody_owner
        || position.expected_revision() != header.expected_custody_position_revision
        || aggregate.direction() != DeltaDirectionV3::Debit
        || aggregate.magnitude() != header.quantity
        || row.position_index() != 0
        || row.outcome() != header.selected_outcome
        || row.delta().direction() != DeltaDirectionV3::Debit
        || row.delta().magnitude() != header.quantity
        || evidence.receipt.packet_digest() != evidence.packet_digest
        || evidence.receipt.claims_program() != evidence.claims_program
        || evidence.receipt.pre_market_revision() != header.expected_claims_market_revision
        || evidence.receipt.post_market_revision() != post_market_revision
    {
        return Err(Error::ClaimsMismatch);
    }
    Ok(SignedDeltaTerminalCompletionV3 {
        effect_digest: evidence.packet_digest,
        resource_digest: evidence.receipt.post_resource_digest(),
        market_revision: post_market_revision,
        custody_position_revision: post_position_revision,
    })
}

fn decode_action(value: u8) -> Result<RepresentationActionV2> {
    match value {
        ACTION_DENOMINATE => Ok(RepresentationActionV2::Denominate),
        ACTION_RECONSTITUTE => Ok(RepresentationActionV2::Reconstitute),
        ACTION_ISSUE_STRUCTURED => Ok(RepresentationActionV2::IssueStructured),
        ACTION_UNWRAP_STRUCTURED => Ok(RepresentationActionV2::UnwrapStructured),
        ACTION_REDEEM_TERMINAL => Ok(RepresentationActionV2::RedeemTerminal),
        _ => Err(Error::NonCanonical),
    }
}

fn decode_role(value: u8) -> Result<CallerRoleV2> {
    match value {
        CALLER_ROLE_CORE => Ok(CallerRoleV2::Core),
        CALLER_ROLE_TRADING => Ok(CallerRoleV2::Trading),
        _ => Err(Error::NonCanonical),
    }
}
