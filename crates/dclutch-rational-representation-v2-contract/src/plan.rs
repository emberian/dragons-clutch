//! Pure composition of canonical Claims and ordered Token effects.

use dclutch_claims_svm::{
    CLAIM_QUANTITY_BYTES, CLAIMS_PLAN_HEADER_BYTES_V1, CallerRole, ClaimsAction, ClaimsPlanV1,
    NO_POSITION_REVISION,
};
use dclutch_rational_representation_v2_kernel::{RepresentationGraphV2, StructuredProjectionV2};

use crate::{
    Error, Result, is_zero,
    request::{AssetV2, CallerRoleV2, RepresentationActionV2, RepresentationRequestV2},
};

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

    /// Exact little-endian quantity-tail width for the canonical Claims plan.
    pub fn claims_quantity_bytes(self) -> Result<usize> {
        usize::try_from(self.request.header().outcome_count)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(CLAIM_QUANTITY_BYTES)
            .ok_or(Error::InvalidLength)
    }

    /// Fill caller-owned scratch with the exact one-hot native Claims vector.
    /// Structured issue/unwrap has no Claims plan and refuses this projection.
    pub fn write_claims_quantities(self, output: &mut [u8]) -> Result<()> {
        if !self.request.header().action.uses_claims()
            || output.len() != self.claims_quantity_bytes()?
        {
            return Err(Error::InvalidActionShape);
        }
        output.fill(0);
        let offset = usize::try_from(self.request.header().selected_outcome)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(CLAIM_QUANTITY_BYTES)
            .ok_or(Error::InvalidLength)?;
        output
            .get_mut(
                offset
                    ..offset
                        .checked_add(CLAIM_QUANTITY_BYTES)
                        .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&self.request.header().quantity.to_le_bytes());
        Ok(())
    }

    /// Construct the one canonical Claims plan. `request_digest` is SHA-256 of
    /// the complete rational representation request bytes and is therefore the
    /// immediate downstream replay coordinate.
    pub fn claims_plan<'b>(
        self,
        request_digest: [u8; 32],
        quantities: &'b [u8],
    ) -> Result<Option<ClaimsPlanV1<'b>>> {
        if is_zero(request_digest) {
            return Err(Error::ZeroIdentity);
        }
        let header = self.request.header();
        if !header.action.uses_claims() {
            if quantities.is_empty() {
                return Ok(None);
            }
            return Err(Error::InvalidActionShape);
        }
        if quantities.len() != self.claims_quantity_bytes()? {
            return Err(Error::InvalidLength);
        }
        let selected_offset = usize::try_from(header.selected_outcome)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(CLAIM_QUANTITY_BYTES)
            .ok_or(Error::InvalidLength)?;
        let mut outcome = 0_u32;
        while outcome < header.outcome_count {
            let offset = usize::try_from(outcome)
                .map_err(|_| Error::InvalidWidth)?
                .checked_mul(CLAIM_QUANTITY_BYTES)
                .ok_or(Error::InvalidLength)?;
            let value = u64::from_le_bytes(
                quantities
                    .get(offset..offset + CLAIM_QUANTITY_BYTES)
                    .ok_or(Error::InvalidLength)?
                    .try_into()
                    .map_err(|_| Error::InvalidLength)?,
            );
            let expected = if offset == selected_offset {
                header.quantity
            } else {
                0
            };
            if value != expected {
                return Err(Error::ClaimsMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let asset = self.request.asset(0)?;
        let (action, source, destination, source_revision, destination_revision) = match header
            .action
        {
            RepresentationActionV2::Denominate => (
                ClaimsAction::Materialize,
                header.actor,
                asset.claims_custody_owner,
                header.expected_actor_position_revision,
                header.expected_custody_position_revision,
            ),
            RepresentationActionV2::Reconstitute => (
                ClaimsAction::Dematerialize,
                asset.claims_custody_owner,
                header.actor,
                header.expected_custody_position_revision,
                header.expected_actor_position_revision,
            ),
            RepresentationActionV2::RedeemTerminal => (
                ClaimsAction::RedeemMaterializedTerminal,
                asset.claims_custody_owner,
                [0; 32],
                header.expected_custody_position_revision,
                NO_POSITION_REVISION,
            ),
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured => {
                return Err(Error::InvalidActionShape);
            }
        };
        let caller = match header.caller_role {
            CallerRoleV2::Core => CallerRole::Core,
            CallerRoleV2::Trading => CallerRole::Trading,
        };
        ClaimsPlanV1::new(
            action,
            caller,
            header.release_set,
            header.market,
            request_digest,
            source,
            destination,
            header.expected_claims_market_revision,
            source_revision,
            destination_revision,
            header.outcome_count,
            quantities,
        )
        .map(Some)
        .map_err(|_| Error::ClaimsMismatch)
    }

    /// Exact byte width of the downstream Claims plan, or zero when inactive.
    pub fn claims_plan_bytes(self) -> Result<usize> {
        if self.request.header().action.uses_claims() {
            CLAIMS_PLAN_HEADER_BYTES_V1
                .checked_add(self.claims_quantity_bytes()?)
                .ok_or(Error::InvalidLength)
        } else {
            Ok(0)
        }
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
    projection: StructuredProjectionV2<'a>,
    graph: RepresentationGraphV2<'a>,
) -> Result<PreparedRepresentationV2<'a>> {
    let header = request.header();
    if graph.graph_id() != header.graph_id
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
        if asset.coefficient != coordinate.coefficient
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
                        authority: header.representation_authority,
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
            authority: header.representation_authority,
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
