//! Pure composition of canonical Claims and ordered Token effects.

use dclutch_claims_svm::{
    CallerRole,
    affine_batch_v2::{
        AffineBatchPlanInputV2, AffineBatchPlanV2, AffineBatchPositionV2, AffineBatchRowInputV2,
        AffineBatchRowV2, DeltaDirectionV2, SignedMagnitudeV2, plan_bytes,
    },
};
use dclutch_rational_representation_v2_kernel::{
    RepresentationDescriptorV2, RepresentationGraphV2, StructuredProjectionV2,
};

use crate::{
    Error, Result,
    request::{AssetV2, CallerRoleV2, RepresentationActionV2, RepresentationRequestV2},
};

/// Finalized Product/LiabilityBasis identities authenticated by the physical
/// adapter before it constructs the canonical affine Claims packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchContextV2 {
    /// Exact finalized Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Exact semantic LiabilityBasisV2 identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized linked-basis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
}

/// Canonical Token effect style. The SBF adapter refines these intents to the
/// exact accepted Token/Token-2022 profile and proves pre/post account deltas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenEffectStyleV2 {
    /// Mint exact shard atoms to the actor.
    MintShard,
    /// Permissioned-burn exact shard atoms from the actor.
    BurnShard,
    /// Transfer exact shard atoms from the actor into Structured custody.
    TransferShardToStructured,
    /// Transfer exact shard atoms from Structured custody to the actor.
    TransferShardFromStructured,
    /// Mint Structured receipt atoms to the actor.
    MintReceipt,
    /// Permissioned-burn Structured receipt atoms from the actor.
    BurnReceipt,
}

/// One exact ordered Token effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenEffectV2 {
    /// Physical effect style.
    pub style: TokenEffectStyleV2,
    /// Token-owned Mint.
    pub mint: [u8; 32],
    /// Source Token Account, zero for mint.
    pub source: [u8; 32],
    /// Destination Token Account, zero for burn.
    pub destination: [u8; 32],
    /// Exact signer/delegate/permissioned-burn authority.
    pub authority: [u8; 32],
    /// Positive raw token atoms.
    pub amount: u64,
}

/// Fully joined request, graph, and Token/Claims projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedRepresentationV2<'a> {
    request: RepresentationRequestV2<'a>,
    projection: StructuredProjectionV2<'a>,
}

impl<'a> PreparedRepresentationV2<'a> {
    /// Exact source request.
    pub const fn request(self) -> RepresentationRequestV2<'a> {
        self.request
    }

    /// Validated Token/Claims projection.
    pub const fn projection(self) -> StructuredProjectionV2<'a> {
        self.projection
    }

    /// Exact byte width of the canonical affine packet. Structured actions
    /// have no Claims effect. Terminal completion remains unavailable until its
    /// distinct typed LiabilityBasisV2 evidence ABI is public.
    pub fn affine_packet_bytes(self) -> Result<usize> {
        match self.request.header().action {
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute => {
                plan_bytes(2, 1).map_err(|_| Error::ClaimsMismatch)
            }
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured => {
                Ok(0)
            }
            RepresentationActionV2::RedeemTerminal => Err(Error::InvalidActionShape),
        }
    }

    /// Write and hostile-decode the sole canonical affine Claims packet.
    /// Context must be present exactly for Denominate/Reconstitute and absent
    /// for Structured actions.
    /// `request_digest` is SHA-256 of the complete rational representation
    /// request bytes and therefore the immediate downstream replay coordinate.
    pub fn write_affine_packet<'b>(
        self,
        request_digest: [u8; 32],
        context: Option<AffineBatchContextV2>,
        output: &'b mut [u8],
    ) -> Result<Option<AffineBatchPlanV2<'b>>> {
        let header = self.request.header();
        match header.action {
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured => {
                if context.is_none() && output.is_empty() {
                    return Ok(None);
                }
                return Err(Error::InvalidActionShape);
            }
            RepresentationActionV2::RedeemTerminal => {
                return Err(Error::InvalidActionShape);
            }
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute => {}
        }
        if output.len() != self.affine_packet_bytes()? {
            return Err(Error::InvalidLength);
        }
        let context = context.ok_or(Error::InvalidActionShape)?;
        let asset = self.request.asset(0)?;
        let positions = [
            AffineBatchPositionV2::new(header.actor, header.expected_actor_position_revision)
                .map_err(|_| Error::ClaimsMismatch)?,
            AffineBatchPositionV2::new(
                asset.claims_custody_owner,
                header.expected_custody_position_revision,
            )
            .map_err(|_| Error::ClaimsMismatch)?,
        ];
        let (source_position_index, destination_position_index) = match header.action {
            RepresentationActionV2::Denominate => (0, 1),
            RepresentationActionV2::Reconstitute => (1, 0),
            RepresentationActionV2::IssueStructured
            | RepresentationActionV2::UnwrapStructured
            | RepresentationActionV2::RedeemTerminal => {
                return Err(Error::InvalidActionShape);
            }
        };
        let rows = [AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: header.selected_outcome,
                source_position_index,
                destination_position_index,
                aggregate_delta: SignedMagnitudeV2::new(DeltaDirectionV2::Neutral, 0)
                    .map_err(|_| Error::ClaimsMismatch)?,
                source_delta: SignedMagnitudeV2::new(DeltaDirectionV2::Debit, header.quantity)
                    .map_err(|_| Error::ClaimsMismatch)?,
                destination_delta: SignedMagnitudeV2::new(
                    DeltaDirectionV2::Credit,
                    header.quantity,
                )
                .map_err(|_| Error::ClaimsMismatch)?,
            },
            header.outcome_count,
            2,
        )
        .map_err(|_| Error::ClaimsMismatch)?];
        let caller_role = match header.caller_role {
            CallerRoleV2::Core => CallerRole::Core,
            CallerRoleV2::Trading => CallerRole::Trading,
        };
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role,
                release_set: header.release_set,
                market: header.market,
                request_id: request_digest,
                product_record_digest: context.product_record_digest,
                semantic_basis_id: context.semantic_basis_id,
                linked_basis_record_digest: context.linked_basis_record_digest,
                expected_market_revision: header.expected_claims_market_revision,
                outcome_count: header.outcome_count,
            },
            &positions,
            &rows,
            output,
        )
        .map_err(|_| Error::ClaimsMismatch)?;
        AffineBatchPlanV2::decode(output)
            .map(Some)
            .map_err(|_| Error::ClaimsMismatch)
    }

    /// Ordered allocation-free Token effect stream.
    pub const fn token_effects(self) -> TokenEffectIterV2<'a> {
        TokenEffectIterV2 {
            prepared: self,
            cursor: 0,
        }
    }
}

/// Join one exact request to the accepted graph and ephemeral Token/Claims
/// projection. No balance is copied into a protocol-owned state.
pub fn prepare<'a>(
    request: RepresentationRequestV2<'a>,
    descriptor: RepresentationDescriptorV2<'_>,
    projection: StructuredProjectionV2<'a>,
    graph: RepresentationGraphV2<'a>,
) -> Result<PreparedRepresentationV2<'a>> {
    let header = request.header();
    descriptor
        .authenticate_graph(graph)
        .map_err(|_| Error::ProjectionMismatch)?;
    if descriptor.descriptor_id() != header.descriptor_id
        || descriptor.graph_id() != header.graph_id
        || descriptor.market_id() != header.market
        || descriptor.release_set_id() != header.release_set
        || descriptor.receipt_mint() != header.receipt_mint
        || descriptor.token_program() != header.token_program
        || descriptor.representation_authority() != header.representation_authority
        || descriptor.outcome_count() != header.outcome_count
        || descriptor.denominator() != header.denominator
        || graph.graph_id() != header.graph_id
        || graph.outcome_count() != header.outcome_count
        || projection.descriptor_id() != header.descriptor_id
        || projection.market_id() != header.market
        || projection.receipt_mint() != header.receipt_mint
        || projection.outcome_count() != header.outcome_count
        || projection.denominator() != header.denominator
        || projection.receipt_supply() != header.expected_receipt_supply
        || projection.revision() != header.expected_representation_revision
    {
        return Err(Error::ProjectionMismatch);
    }
    let mut index = 0_u32;
    while index < header.asset_count {
        let outcome = if header.action.selected_outcome() {
            header.selected_outcome
        } else {
            index
        };
        let asset = request.asset(index)?;
        let coordinate = projection
            .coordinate(outcome)
            .map_err(|_| Error::ProjectionMismatch)?;
        if asset.coefficient
            != descriptor
                .coefficient(outcome)
                .map_err(|_| Error::ProjectionMismatch)?
            || asset.coefficient != coordinate.coefficient
            || asset.expected_shard_supply != coordinate.shard_supply
            || asset.expected_structured_shards != coordinate.structured_custody
            || asset.expected_actor_shards > coordinate.explicit_free_shards
        {
            return Err(Error::ProjectionMismatch);
        }
        let effect_amount = match header.action {
            RepresentationActionV2::Denominate
            | RepresentationActionV2::Reconstitute
            | RepresentationActionV2::RedeemTerminal => header
                .denominator
                .checked_mul(header.quantity)
                .ok_or(Error::ArithmeticOverflow)?,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured => {
                asset
                    .coefficient
                    .checked_mul(header.quantity)
                    .ok_or(Error::ArithmeticOverflow)?
            }
        };
        let enough = match header.action {
            RepresentationActionV2::Reconstitute
            | RepresentationActionV2::RedeemTerminal
            | RepresentationActionV2::IssueStructured => {
                asset.expected_actor_shards >= effect_amount
            }
            RepresentationActionV2::UnwrapStructured => {
                asset.expected_structured_shards >= effect_amount
            }
            RepresentationActionV2::Denominate => true,
        };
        if !enough {
            return Err(Error::InsufficientBalance);
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    if header.action == RepresentationActionV2::UnwrapStructured
        && header.quantity > header.expected_receipt_supply
    {
        return Err(Error::InsufficientBalance);
    }
    Ok(PreparedRepresentationV2 {
        request,
        projection,
    })
}

/// Allocation-free exact effect iterator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenEffectIterV2<'a> {
    prepared: PreparedRepresentationV2<'a>,
    cursor: u32,
}

impl Iterator for TokenEffectIterV2<'_> {
    type Item = Result<TokenEffectV2>;

    fn next(&mut self) -> Option<Self::Item> {
        let header = self.prepared.request.header();
        let result = match header.action {
            RepresentationActionV2::Denominate => {
                if self.cursor != 0 {
                    return None;
                }
                self.selected_effect(TokenEffectStyleV2::MintShard, true)
            }
            RepresentationActionV2::Reconstitute | RepresentationActionV2::RedeemTerminal => {
                if self.cursor != 0 {
                    return None;
                }
                self.selected_effect(TokenEffectStyleV2::BurnShard, false)
            }
            RepresentationActionV2::IssueStructured => {
                if self.cursor < header.asset_count {
                    self.structured_transfer(true)
                } else if self.cursor == header.asset_count {
                    Ok(TokenEffectV2 {
                        style: TokenEffectStyleV2::MintReceipt,
                        mint: header.receipt_mint,
                        source: [0; 32],
                        destination: header.receipt_account,
                        authority: header.representation_authority,
                        amount: header.quantity,
                    })
                } else {
                    return None;
                }
            }
            RepresentationActionV2::UnwrapStructured => {
                if self.cursor == 0 {
                    Ok(TokenEffectV2 {
                        style: TokenEffectStyleV2::BurnReceipt,
                        mint: header.receipt_mint,
                        source: header.receipt_account,
                        destination: [0; 32],
                        authority: header.actor,
                        amount: header.quantity,
                    })
                } else if self.cursor <= header.asset_count {
                    self.structured_transfer(false)
                } else {
                    return None;
                }
            }
        };
        self.cursor = match self.cursor.checked_add(1) {
            Some(value) => value,
            None => return Some(Err(Error::ArithmeticOverflow)),
        };
        Some(result)
    }
}

impl TokenEffectIterV2<'_> {
    fn selected_effect(self, style: TokenEffectStyleV2, minting: bool) -> Result<TokenEffectV2> {
        let header = self.prepared.request.header();
        let asset = self.prepared.request.asset(0)?;
        let amount = header
            .denominator
            .checked_mul(header.quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(TokenEffectV2 {
            style,
            mint: asset.shard_mint,
            source: if minting {
                [0; 32]
            } else {
                asset.actor_shard_account
            },
            destination: if minting {
                asset.actor_shard_account
            } else {
                [0; 32]
            },
            authority: if minting {
                header.representation_authority
            } else {
                header.actor
            },
            amount,
        })
    }

    fn structured_transfer(self, issuing: bool) -> Result<TokenEffectV2> {
        let header = self.prepared.request.header();
        let asset_index = if issuing {
            self.cursor
        } else {
            self.cursor
                .checked_sub(1)
                .ok_or(Error::ArithmeticOverflow)?
        };
        let asset: AssetV2 = self.prepared.request.asset(asset_index)?;
        let amount = asset
            .coefficient
            .checked_mul(header.quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(TokenEffectV2 {
            style: if issuing {
                TokenEffectStyleV2::TransferShardToStructured
            } else {
                TokenEffectStyleV2::TransferShardFromStructured
            },
            mint: asset.shard_mint,
            source: if issuing {
                asset.actor_shard_account
            } else {
                asset.structured_custody_account
            },
            destination: if issuing {
                asset.structured_custody_account
            } else {
                asset.actor_shard_account
            },
            authority: if issuing {
                header.actor
            } else {
                header.representation_authority
            },
            amount,
        })
    }
}
