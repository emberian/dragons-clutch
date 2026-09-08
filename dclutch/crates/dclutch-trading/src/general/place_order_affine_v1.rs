//! Canonicalize General PlaceOrder's private Claims affine child packet.
//!
//! The General outer frame retains semantic positions: maker first and the
//! lifecycle-created order escrow second.  Claims requires its affine table
//! and supplied Position accounts in increasing PDA-key order.  This adapter
//! is the one narrow boundary between those two orderings.  It preserves every
//! header fact and every row delta, changing only the two table positions and
//! the row indices that address them.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims::affine_batch_v2::{
    AffineBatchPlanInputV2, AffineBatchPlanV2, AffineBatchRowInputV2, AffineBatchRowV2,
    canonicalize_zero_transfer_rows_into_v2,
};

/// The typed PlaceOrder affine canonicalizer could not preserve an exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralPlaceOrderAffineErrorV1 {
    /// The supplied packet was not one canonical two-Position affine plan.
    Plan,
    /// The semantic maker and escrow positions named the same physical key.
    AliasedPosition,
    /// The bounded output allocation could not be reserved.
    Allocation,
}

/// Map canonical key order back to the semantic `(maker, escrow)` pair.
///
/// Each value is the semantic index whose account must occupy the canonical
/// account slot of the same index.
pub type GeneralPlaceOrderAffineOrderV1 = [u8; 2];

/// Re-encode one semantic `(maker, escrow)` affine plan in canonical PDA-key
/// order and return the matching account permutation.
///
/// `semantic_position_keys[0]` is the maker Position and index one is the
/// order escrow Position.  The caller applies the returned permutation to its
/// two gathered `AccountInfo`s; the returned bytes and that reordered pair are
/// then one Claims instruction, so the caller-authority digest binds exactly
/// what Claims receives.
pub fn canonicalize_general_place_order_affine_v1(
    request: &[u8],
    semantic_position_keys: [[u8; 32]; 2],
    output: &mut Vec<u8>,
) -> Result<GeneralPlaceOrderAffineOrderV1, GeneralPlaceOrderAffineErrorV1> {
    if semantic_position_keys[0] == semantic_position_keys[1] {
        return Err(GeneralPlaceOrderAffineErrorV1::AliasedPosition);
    }
    // EffectProgram materializes one fixed affine row for every Product
    // outcome. A selected Sell owns claims only at its signed interval, so
    // unused rows arrive as the known Debit/Credit-zero producer placeholder.
    // Compact those rows before the strict Claims decoder and before the child
    // digest/caller authority are derived. The public on-chain decoder still
    // rejects the raw placeholder; this is only the typed producer boundary.
    output.clear();
    output
        .try_reserve_exact(request.len())
        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Allocation)?;
    output.resize(request.len(), 0);
    let normalized_len = canonicalize_zero_transfer_rows_into_v2(request, output.as_mut_slice())
        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?;
    output.truncate(normalized_len);
    let plan = AffineBatchPlanV2::decode(output.as_slice())
        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?;
    if plan.position_count() != 2 {
        return Err(GeneralPlaceOrderAffineErrorV1::Plan);
    }

    let order = if semantic_position_keys[0] < semantic_position_keys[1] {
        [0_u8, 1]
    } else {
        [1_u8, 0]
    };
    let mut semantic_to_canonical = [0_u32; 2];
    semantic_to_canonical[usize::from(order[0])] = 0;
    semantic_to_canonical[usize::from(order[1])] = 1;

    let positions = [
        plan.position(u32::from(order[0]))
            .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?,
        plan.position(u32::from(order[1]))
            .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?,
    ];
    let row_count =
        usize::try_from(plan.row_count()).map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?;
    let header = AffineBatchPlanInputV2 {
        caller_role: plan.caller_role(),
        release_set: plan.release_set(),
        market: plan.market(),
        request_id: plan.request_id(),
        product_record_digest: plan.product_record_digest(),
        semantic_basis_id: plan.semantic_basis_id(),
        linked_basis_record_digest: plan.linked_basis_record_digest(),
        expected_market_revision: plan.expected_market_revision(),
        outcome_count: plan.outcome_count(),
    };
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count)
        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Allocation)?;
    let mut index = 0_u32;
    while index < plan.row_count() {
        let row = plan
            .row(index)
            .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?;
        let source_position_index = if row.source_present() {
            *semantic_to_canonical
                .get(
                    usize::try_from(row.source_position_index())
                        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?,
                )
                .ok_or(GeneralPlaceOrderAffineErrorV1::Plan)?
        } else {
            0
        };
        let destination_position_index = if row.destination_present() {
            *semantic_to_canonical
                .get(
                    usize::try_from(row.destination_position_index())
                        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?,
                )
                .ok_or(GeneralPlaceOrderAffineErrorV1::Plan)?
        } else {
            0
        };
        rows.push(
            AffineBatchRowV2::new(
                AffineBatchRowInputV2 {
                    source_present: row.source_present(),
                    destination_present: row.destination_present(),
                    outcome: row.outcome(),
                    source_position_index,
                    destination_position_index,
                    aggregate_delta: row.aggregate_delta(),
                    source_delta: row.source_delta(),
                    destination_delta: row.destination_delta(),
                },
                plan.outcome_count(),
                2,
            )
            .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?,
        );
        index = index
            .checked_add(1)
            .ok_or(GeneralPlaceOrderAffineErrorV1::Plan)?;
    }

    output.clear();
    output
        .try_reserve_exact(normalized_len)
        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Allocation)?;
    output.resize(normalized_len, 0);
    AffineBatchPlanV2::encode_into(header, &positions, &rows, output)
        .map_err(|_| GeneralPlaceOrderAffineErrorV1::Plan)?;
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use dclutch_claims::{
        CallerRole,
        affine_batch_v2::{
            AFFINE_BATCH_PLAN_HEADER_BYTES_V2, AFFINE_BATCH_POSITION_BYTES_V2,
            AFFINE_BATCH_ROW_BYTES_V2, AffineBatchErrorV2, AffineBatchPositionV2,
            AffineBatchRequestLayoutV2, DeltaDirectionV2, SignedMagnitudeV2,
        },
    };

    fn magnitude(direction: DeltaDirectionV2, amount: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(direction, amount).expect("canonical magnitude")
    }

    fn semantic_plan() -> Vec<u8> {
        let positions = [
            AffineBatchPositionV2::new([0x11; 32], 4).expect("maker Position"),
            AffineBatchPositionV2::new([0x22; 32], 9).expect("escrow Position"),
        ];
        let rows = [AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: magnitude(DeltaDirectionV2::Neutral, 0),
                source_delta: magnitude(DeltaDirectionV2::Debit, 3),
                destination_delta: magnitude(DeltaDirectionV2::Credit, 3),
            },
            1,
            2,
        )
        .expect("semantic row")];
        let mut bytes = vec![
            0_u8;
            AFFINE_BATCH_PLAN_HEADER_BYTES_V2
                + 2 * AFFINE_BATCH_POSITION_BYTES_V2
                + AFFINE_BATCH_ROW_BYTES_V2
        ];
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 7,
                outcome_count: 1,
            },
            &positions,
            &rows,
            &mut bytes,
        )
        .expect("semantic plan");
        bytes
    }

    #[test]
    fn canonicalizes_both_pda_orders_and_preserves_row_meaning() {
        for (keys, expected_order, expected_owners) in [
            ([[1; 32], [2; 32]], [0, 1], [[0x11; 32], [0x22; 32]]),
            ([[2; 32], [1; 32]], [1, 0], [[0x22; 32], [0x11; 32]]),
        ] {
            let request = semantic_plan();
            let mut output = Vec::new();
            assert_eq!(
                canonicalize_general_place_order_affine_v1(&request, keys, &mut output)
                    .expect("canonical PlaceOrder child"),
                expected_order
            );
            let plan = AffineBatchPlanV2::decode(&output).expect("canonical plan");
            assert_eq!(plan.position(0).expect("first").owner(), expected_owners[0]);
            assert_eq!(
                plan.position(1).expect("second").owner(),
                expected_owners[1]
            );
            let row = plan.row(0).expect("row");
            if expected_order == [0, 1] {
                assert_eq!(row.source_position_index(), 0);
                assert_eq!(row.destination_position_index(), 1);
            } else {
                assert_eq!(row.source_position_index(), 1);
                assert_eq!(row.destination_position_index(), 0);
            }
            assert_eq!(row.source_delta().magnitude(), 3);
            assert_eq!(row.destination_delta().magnitude(), 3);
        }
    }

    #[test]
    fn compacts_mixed_sell_zero_rows_before_key_sort_and_child_digest() {
        let positions = [
            AffineBatchPositionV2::new([0x11; 32], 4).expect("maker Position"),
            AffineBatchPositionV2::new([0x22; 32], 9).expect("escrow Position"),
        ];
        let rows = [
            AffineBatchRowV2::new(
                AffineBatchRowInputV2 {
                    source_present: true,
                    destination_present: true,
                    outcome: 0,
                    source_position_index: 0,
                    destination_position_index: 1,
                    aggregate_delta: magnitude(DeltaDirectionV2::Neutral, 0),
                    source_delta: magnitude(DeltaDirectionV2::Debit, 3),
                    destination_delta: magnitude(DeltaDirectionV2::Credit, 3),
                },
                2,
                2,
            )
            .expect("first row"),
            AffineBatchRowV2::new(
                AffineBatchRowInputV2 {
                    source_present: true,
                    destination_present: true,
                    outcome: 1,
                    source_position_index: 0,
                    destination_position_index: 1,
                    aggregate_delta: magnitude(DeltaDirectionV2::Neutral, 0),
                    source_delta: magnitude(DeltaDirectionV2::Debit, 3),
                    destination_delta: magnitude(DeltaDirectionV2::Credit, 3),
                },
                2,
                2,
            )
            .expect("selected row"),
        ];
        let mut raw = vec![
            0_u8;
            AFFINE_BATCH_PLAN_HEADER_BYTES_V2
                + 2 * AFFINE_BATCH_POSITION_BYTES_V2
                + 2 * AFFINE_BATCH_ROW_BYTES_V2
        ];
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 7,
                outcome_count: 2,
            },
            &positions,
            &rows,
            &mut raw,
        )
        .expect("two-row template");
        let first_row = AFFINE_BATCH_PLAN_HEADER_BYTES_V2 + 2 * AFFINE_BATCH_POSITION_BYTES_V2;
        for offset in [
            AffineBatchRequestLayoutV2::ROW_SOURCE_MAGNITUDE,
            AffineBatchRequestLayoutV2::ROW_DESTINATION_MAGNITUDE,
        ] {
            raw[first_row + offset..first_row + offset + 8].copy_from_slice(&0_u64.to_le_bytes());
        }
        assert_eq!(
            AffineBatchPlanV2::decode(&raw),
            Err(AffineBatchErrorV2::InvalidDelta),
            "the raw expanded producer rows are never accepted by Claims"
        );

        let mut output = Vec::new();
        assert_eq!(
            canonicalize_general_place_order_affine_v1(&raw, [[2; 32], [1; 32]], &mut output,)
                .expect("General normalizes before its key-sorted child commitment"),
            [1, 0]
        );
        let plan = AffineBatchPlanV2::decode(&output).expect("strict compact child");
        assert_eq!(plan.row_count(), 1);
        let row = plan.row(0).expect("retained Sell row");
        assert_eq!(row.outcome(), 1);
        assert_eq!(row.source_delta().direction(), DeltaDirectionV2::Debit);
        assert_eq!(row.source_delta().magnitude(), 3);
        assert_eq!(
            row.destination_delta().direction(),
            DeltaDirectionV2::Credit
        );
        assert_eq!(row.destination_delta().magnitude(), 3);
        assert_eq!(row.source_position_index(), 1);
        assert_eq!(row.destination_position_index(), 0);
    }
}
