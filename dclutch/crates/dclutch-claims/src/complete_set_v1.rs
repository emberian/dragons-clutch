//! The ONE LiabilityBasisV2 complete-set executor.
//!
//! # What this is
//!
//! `signed_delta_v3` and `affine_batch_v2` each open-coded a private
//! `apply_coordinate` -- one `u64` read, one checked add or sub, one write at
//! `header + outcome * 8` -- and nothing in the tree minted or burned a uniform
//! vector against a live `DCLLBM02` aggregate. The conservation route reached
//! for the economic slice's `execute_basket` instead, which decodes a Market
//! family (`DCLTEMK2`) no founding writes, and CLAIMS-18 proved on the shipped
//! ELF that the route could not execute at all.
//!
//! This module is the ruling's executor (orchestrator, 2026-09-05): one author
//! for the coordinate write, and on top of it one author for the complete-set
//! act -- `Mint` and `Merge`, categorical and refunding -- over an LBV2
//! aggregate, a holder Position and, when the record refunds, the Market's
//! failure escrow. `signed_delta_v3`, `affine_batch_v2` and
//! `claims_conservation_v1` call it; none of them spells the arithmetic again.
//!
//! # Where the principal lives
//!
//! An LBV2 aggregate carries NO Hoard scalar. Under the ruling an LBV2
//! Market's outstanding principal lives in the Custody `HoardPrincipal` vault
//! -- the token account -- and nowhere in a header. So this executor never
//! reads or writes a principal: it moves claims, and [`principal_v1`] states
//! the one relation between the vault and the supply vector that every
//! complete-set act must find true before it runs and leave true after:
//!
//! ```text
//!   vault_atoms >= max_k supply[k] * basis_scale        (L4, the LBV2 form)
//! ```
//!
//! which for a Market whose supply is uniform (every open Market: the
//! complete-set acts are the only ones that move supply and both move every
//! coordinate alike) is `vault_atoms >= sets * basis_scale`. The Lean instance
//! is `vaultPrincipal` and `the_complete_set_acts_keep_the_vault_backing` in
//! `formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean`.
//!
//! An INEQUALITY, not an equality, and on purpose: a token account accepts a
//! transfer from anybody, so an equality would let a stranger's one-atom
//! donation to the vault refuse every split and merge on the Market forever.
//! Excess backing is harmless; a shortfall is the refusal.
//!
//! # The semantics, by theorem
//!
//! The categorical act is the kernel's `splitPost` / `mergePost`
//! (`split_complete_set_exact`, `merge_complete_set_exact`,
//! `merge_complete_set_conserves_backing`). The refunding act is
//! `refundingSplitPost` / `refundingMergePost`: the aggregate moves exactly as
//! the categorical act moves it
//! (`escrowed_founding_is_a_complete_set_split_in_the_aggregate`,
//! `refunding_merge_is_a_complete_set_merge_in_the_aggregate`), the holder
//! moves at the ordinary coordinates and the escrow at the failure coordinate
//! (`the_refunding_split_spreads_one_set_across_two_positions`,
//! `the_refunding_merge_burns_one_set_across_two_positions`), the two acts are
//! inverse (`the_refunding_merge_undoes_the_refunding_split`) and both keep
//! the escrow seated (`the_refunding_actions_keep_the_escrow_seated`).
//!
//! The failure selector comes from the economic kernel's
//! `refunding_failure_index`, the sole author of which coordinate a refunding
//! complete set seats where. Nothing here re-spells "the last one".
//!
//! # What it does not do
//!
//! It authenticates no account. Every buffer is a CANDIDATE the caller has
//! already authenticated -- owner, PDA, identity joins, revision pins -- and
//! decides to commit or not. A refusal here leaves every buffer as it was
//! handed in only in the sense that the caller discards the candidates; the
//! executor writes as it goes and does not roll back, which is why callers
//! hand it copies and commit last.

use crate::liability_basis_state_v2::{
    LIABILITY_BASIS_CLAIM_STRIDE_V2, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketLayoutV2,
    LiabilityBasisMarketViewV2, LiabilityBasisPositionLayoutV2, LiabilityBasisPositionViewV2,
    LiabilityBasisStateErrorV2,
};

#[cfg(test)]
extern crate std;

/// Every refusal the executor can raise, each one accusation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteSetErrorV1 {
    /// The aggregate candidate is not a canonical `DCLLBM02` body.
    Aggregate(LiabilityBasisStateErrorV2),
    /// A Position candidate is not a canonical `DCLLBP02` body.
    Position(LiabilityBasisStateErrorV2),
    /// A Position's width or basis does not join the aggregate's.
    Join,
    /// A coordinate offset did not fit the candidate buffer.
    Coordinate,
    /// A zero quantity, which is not a canonical act.
    Quantity,
    /// A credit overflowed `u64` at some coordinate.
    Overflow,
    /// A debit found less than `quantity` at some coordinate: the holder does
    /// not hold a complete set, or the escrow is not seated to it.
    Holding,
    /// A refunding act was asked for with no escrow candidate.
    EscrowRequired,
    /// A categorical act was handed an escrow candidate it may not touch.
    EscrowForbidden,
    /// A revision could not advance by exactly one.
    Revision,
    /// The vault does not back the outstanding supply at the basis scale.
    Backing,
    /// The runtime width seats no failure coordinate.
    Width,
}

/// Result alias for the executor.
pub type Result<T> = core::result::Result<T, CompleteSetErrorV1>;

/// One exact signed movement at one coordinate.
///
/// Neutral at magnitude zero and nowhere else, which is the canon
/// `SignedDeltaV3` and `SignedMagnitudeV2` both already enforce; the
/// conversions below are total because both source types are canonical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateDeltaV1 {
    /// Leave the coordinate as it is.
    Neutral,
    /// Add the magnitude.
    Credit(u64),
    /// Subtract the magnitude.
    Debit(u64),
}

impl From<crate::signed_delta_v3::SignedDeltaV3> for CoordinateDeltaV1 {
    fn from(value: crate::signed_delta_v3::SignedDeltaV3) -> Self {
        use crate::signed_delta_v3::DeltaDirectionV3;
        match value.direction() {
            DeltaDirectionV3::Neutral => Self::Neutral,
            DeltaDirectionV3::Credit => Self::Credit(value.magnitude()),
            DeltaDirectionV3::Debit => Self::Debit(value.magnitude()),
        }
    }
}

impl From<crate::affine_batch_v2::SignedMagnitudeV2> for CoordinateDeltaV1 {
    fn from(value: crate::affine_batch_v2::SignedMagnitudeV2) -> Self {
        use crate::affine_batch_v2::DeltaDirectionV2;
        match value.direction() {
            DeltaDirectionV2::Neutral => Self::Neutral,
            DeltaDirectionV2::Credit => Self::Credit(value.magnitude()),
            DeltaDirectionV2::Debit => Self::Debit(value.magnitude()),
        }
    }
}

/// The one live writer of a claim on a live LBV2 record.
///
/// `header` is the record family's header width -- the aggregate's or a
/// Position's -- and `outcome` the coordinate. The offset arithmetic is
/// checked, the movement is checked, and an underflow is [`Holding`]
/// (the resource did not hold what the debit asked for) while an overflow is
/// [`Overflow`], because a reader who meets the two goes to different places.
///
/// [`Holding`]: CompleteSetErrorV1::Holding
/// [`Overflow`]: CompleteSetErrorV1::Overflow
pub fn apply_coordinate_v1(
    bytes: &mut [u8],
    header: usize,
    outcome: u32,
    delta: CoordinateDeltaV1,
) -> Result<()> {
    let offset = usize::try_from(outcome)
        .ok()
        .and_then(|outcome| outcome.checked_mul(LIABILITY_BASIS_CLAIM_STRIDE_V2))
        .and_then(|relative| header.checked_add(relative))
        .ok_or(CompleteSetErrorV1::Coordinate)?;
    let before = read_u64(bytes, offset)?;
    let after = match delta {
        CoordinateDeltaV1::Neutral => before,
        CoordinateDeltaV1::Credit(magnitude) => before
            .checked_add(magnitude)
            .ok_or(CompleteSetErrorV1::Overflow)?,
        CoordinateDeltaV1::Debit(magnitude) => before
            .checked_sub(magnitude)
            .ok_or(CompleteSetErrorV1::Holding)?,
    };
    put_u64(bytes, offset, after)
}

/// Which way a complete set moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteSetDirectionV1 {
    /// Create one claim at every coordinate against new collateral.
    Mint,
    /// Destroy one claim at every coordinate and release its collateral.
    Merge,
}

impl CompleteSetDirectionV1 {
    const fn delta(self, quantity: u64) -> CoordinateDeltaV1 {
        match self {
            Self::Mint => CoordinateDeltaV1::Credit(quantity),
            Self::Merge => CoordinateDeltaV1::Debit(quantity),
        }
    }
}

/// One complete-set act, fully stated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteSetActV1 {
    /// Mint or merge.
    pub direction: CompleteSetDirectionV1,
    /// Exact complete sets moved; zero refuses.
    pub quantity: u64,
    /// Whether the Market refunds on failure -- the RECORD's answer, read by
    /// the caller off the authenticated basis record. When true the failure
    /// coordinate is seated in the escrow candidate and never in the holder's.
    pub refunding: bool,
}

/// What the executor left behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteSetPostV1 {
    /// Runtime width.
    pub claim_count: u32,
    /// The failure selector the act seated, on a refunding Market.
    pub failure_selector: Option<u32>,
    /// The aggregate's revision after the act.
    pub market_revision: u64,
    /// The holder's revision after the act.
    pub holder_revision: u64,
    /// The escrow's revision after the act, on a refunding Market.
    pub escrow_revision: Option<u64>,
}

/// Apply one complete-set act over candidate buffers.
///
/// `aggregate` is a `DCLLBM02` body, `holder` and `escrow` are `DCLLBP02`
/// bodies. Every coordinate of the aggregate moves by `quantity`; the holder
/// moves at every coordinate on a categorical Market and at every ORDINARY
/// coordinate on a refunding one, where the escrow moves at the failure
/// coordinate instead. All three revisions advance by one.
///
/// The Lean admission is checked here in its executable form: a mint refuses
/// where any credit would overflow, a merge refuses where any resource holds
/// less than `quantity` -- `commandAccepts` for `splitCompleteSet` and
/// `mergeCompleteSet`, with the Hoard term delegated to the vault (see
/// [`principal_v1`]).
pub fn apply_complete_set_v1(
    aggregate: &mut [u8],
    holder: &mut [u8],
    escrow: Option<&mut [u8]>,
    act: CompleteSetActV1,
) -> Result<CompleteSetPostV1> {
    if act.quantity == 0 {
        return Err(CompleteSetErrorV1::Quantity);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(aggregate).map_err(CompleteSetErrorV1::Aggregate)?;
    let holder_view =
        LiabilityBasisPositionViewV2::decode(holder).map_err(CompleteSetErrorV1::Position)?;
    require_join(market, holder_view)?;
    let failure_selector = if act.refunding {
        Some(failure_selector_v1(market.claim_count)?)
    } else {
        None
    };
    let escrow = match (failure_selector, escrow) {
        (Some(_), Some(escrow)) => {
            let escrow_view = LiabilityBasisPositionViewV2::decode(escrow)
                .map_err(CompleteSetErrorV1::Position)?;
            require_join(market, escrow_view)?;
            Some(escrow)
        }
        (Some(_), None) => return Err(CompleteSetErrorV1::EscrowRequired),
        (None, Some(_)) => return Err(CompleteSetErrorV1::EscrowForbidden),
        (None, None) => None,
    };
    let delta = act.direction.delta(act.quantity);
    let mut escrow = escrow;
    for outcome in 0..market.claim_count {
        apply_coordinate_v1(
            aggregate,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            outcome,
            delta,
        )?;
        match (failure_selector, escrow.as_deref_mut()) {
            (Some(failure), Some(escrow)) if outcome == failure => {
                apply_coordinate_v1(
                    escrow,
                    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                    outcome,
                    delta,
                )?;
            }
            _ => {
                apply_coordinate_v1(
                    holder,
                    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                    outcome,
                    delta,
                )?;
            }
        }
    }
    let market_revision = advance_revision(aggregate, LiabilityBasisMarketLayoutV2::REVISION)?;
    let holder_revision = advance_revision(holder, LiabilityBasisPositionLayoutV2::REVISION)?;
    let escrow_revision = match escrow {
        Some(escrow) => Some(advance_revision(
            escrow,
            LiabilityBasisPositionLayoutV2::REVISION,
        )?),
        None => None,
    };
    Ok(CompleteSetPostV1 {
        claim_count: market.claim_count,
        failure_selector,
        market_revision,
        holder_revision,
        escrow_revision,
    })
}

/// The failure coordinate of a refunding Market at one width, from the sole
/// author.
pub fn failure_selector_v1(claim_count: u32) -> Result<u32> {
    let failure = dclutch_product::economic_slice::refunding_failure_index(claim_count)
        .map_err(|_| CompleteSetErrorV1::Width)?;
    u32::try_from(failure).map_err(|_| CompleteSetErrorV1::Width)
}

/// The vault-principal relation, read rather than stored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrincipalV1 {
    /// Complete sets outstanding: the largest supply at any coordinate, which
    /// on an open Market is every coordinate's.
    pub outstanding_sets: u64,
    /// Collateral atoms those sets are backed by at the basis scale.
    pub required_backing_atoms: u64,
    /// What the vault actually holds.
    pub vault_atoms: u64,
}

/// Read the outstanding principal off the aggregate and prove the vault backs
/// it.
///
/// `vault_atoms` is the HoardPrincipal vault's token balance, read by the
/// caller through Custody's own token reader. This is L4 in its LBV2 form and
/// the precondition every complete-set act runs under; both acts move the
/// vault and the supply by the same sets, so it is also the postcondition.
pub fn principal_v1(aggregate: &[u8], vault_atoms: u64, basis_scale: u64) -> Result<PrincipalV1> {
    if basis_scale == 0 {
        return Err(CompleteSetErrorV1::Quantity);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(aggregate).map_err(CompleteSetErrorV1::Aggregate)?;
    let mut outstanding_sets = 0_u64;
    for outcome in 0..market.claim_count {
        let supply = market
            .supply(aggregate, outcome)
            .map_err(CompleteSetErrorV1::Aggregate)?;
        outstanding_sets = outstanding_sets.max(supply);
    }
    let required_backing_atoms = outstanding_sets
        .checked_mul(basis_scale)
        .ok_or(CompleteSetErrorV1::Overflow)?;
    if vault_atoms < required_backing_atoms {
        return Err(CompleteSetErrorV1::Backing);
    }
    Ok(PrincipalV1 {
        outstanding_sets,
        required_backing_atoms,
        vault_atoms,
    })
}

/// How many complete sets one Position can merge: the smallest balance among
/// the coordinates a merge burns from it.
///
/// On a refunding Market the failure coordinate is the escrow's and is not
/// counted; on a categorical one every coordinate counts. Arithmetic on what
/// is held, not an offer.
pub fn held_complete_sets_v1(position: &[u8], refunding: bool) -> Result<u64> {
    let view =
        LiabilityBasisPositionViewV2::decode(position).map_err(CompleteSetErrorV1::Position)?;
    let failure = if refunding {
        Some(failure_selector_v1(view.claim_count)?)
    } else {
        None
    };
    let mut held = u64::MAX;
    for outcome in 0..view.claim_count {
        if failure == Some(outcome) {
            continue;
        }
        let balance = view
            .balance(position, outcome)
            .map_err(CompleteSetErrorV1::Position)?;
        held = held.min(balance);
    }
    Ok(held)
}

fn require_join(
    market: LiabilityBasisMarketViewV2,
    position: LiabilityBasisPositionViewV2,
) -> Result<()> {
    if position.claim_count != market.claim_count || position.basis_id != market.basis_id {
        return Err(CompleteSetErrorV1::Join);
    }
    Ok(())
}

fn advance_revision(bytes: &mut [u8], offset: usize) -> Result<u64> {
    let next = read_u64(bytes, offset)?
        .checked_add(1)
        .ok_or(CompleteSetErrorV1::Revision)?;
    put_u64(bytes, offset, next)?;
    Ok(next)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(LIABILITY_BASIS_CLAIM_STRIDE_V2)
        .ok_or(CompleteSetErrorV1::Coordinate)?;
    let field: [u8; LIABILITY_BASIS_CLAIM_STRIDE_V2] = bytes
        .get(offset..end)
        .ok_or(CompleteSetErrorV1::Coordinate)?
        .try_into()
        .map_err(|_| CompleteSetErrorV1::Coordinate)?;
    Ok(u64::from_le_bytes(field))
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let end = offset
        .checked_add(LIABILITY_BASIS_CLAIM_STRIDE_V2)
        .ok_or(CompleteSetErrorV1::Coordinate)?;
    bytes
        .get_mut(offset..end)
        .ok_or(CompleteSetErrorV1::Coordinate)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liability_basis_state_v2::{
        LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
        liability_basis_vector_width_v2,
    };
    use std::vec;
    use std::vec::Vec;

    const WIDTH: u32 = 4;
    const BASIS: [u8; 32] = [0x15; 32];

    fn aggregate(supplies: &[u64], revision: u64) -> Vec<u8> {
        let mut bytes = vec![
            0_u8;
            liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, WIDTH)
                .unwrap()
        ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision,
                logical_market: [1; 32],
                release_set: [2; 32],
                registry_program: [3; 32],
                product_instance_id: [4; 32],
                basis_id: BASIS,
                realm_id: [6; 32],
                custody_context: [7; 32],
                generation: 9,
            },
            supplies,
            &mut bytes,
        )
        .unwrap();
        bytes
    }

    fn position(owner: u8, balances: &[u64], revision: u64) -> Vec<u8> {
        let mut bytes = vec![
            0_u8;
            liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, WIDTH)
                .unwrap()
        ];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision,
                market_account: [0x0a; 32],
                owner: [owner; 32],
                basis_id: BASIS,
            },
            balances,
            &mut bytes,
        )
        .unwrap();
        bytes
    }

    fn supplies(bytes: &[u8]) -> Vec<u64> {
        let view = LiabilityBasisMarketViewV2::decode(bytes).unwrap();
        (0..WIDTH).map(|k| view.supply(bytes, k).unwrap()).collect()
    }

    fn balances(bytes: &[u8]) -> Vec<u64> {
        let view = LiabilityBasisPositionViewV2::decode(bytes).unwrap();
        (0..WIDTH).map(|k| view.balance(bytes, k).unwrap()).collect()
    }

    /// `split_complete_set_exact` and `merge_complete_set_exact`, executed:
    /// every coordinate of the aggregate and the holder moves by the quantity,
    /// and the merge undoes the mint to the byte except for the two revision
    /// advances.
    #[test]
    fn a_categorical_mint_then_merge_is_the_identity_but_for_revisions() {
        let before_aggregate = aggregate(&[10; 4], 4);
        let before_holder = position(0x21, &[0; 4], 2);
        let mut agg = before_aggregate.clone();
        let mut holder = before_holder.clone();
        let post = apply_complete_set_v1(
            &mut agg,
            &mut holder,
            None,
            CompleteSetActV1 {
                direction: CompleteSetDirectionV1::Mint,
                quantity: 5,
                refunding: false,
            },
        )
        .unwrap();
        assert_eq!(supplies(&agg), vec![15; 4]);
        assert_eq!(balances(&holder), vec![5; 4]);
        assert_eq!((post.market_revision, post.holder_revision), (5, 3));
        assert_eq!(post.escrow_revision, None);
        let post = apply_complete_set_v1(
            &mut agg,
            &mut holder,
            None,
            CompleteSetActV1 {
                direction: CompleteSetDirectionV1::Merge,
                quantity: 5,
                refunding: false,
            },
        )
        .unwrap();
        assert_eq!(supplies(&agg), supplies(&before_aggregate));
        assert_eq!(balances(&holder), balances(&before_holder));
        assert_eq!((post.market_revision, post.holder_revision), (6, 4));
        let mut expected_aggregate = before_aggregate.clone();
        advance_revision(&mut expected_aggregate, LiabilityBasisMarketLayoutV2::REVISION).unwrap();
        advance_revision(&mut expected_aggregate, LiabilityBasisMarketLayoutV2::REVISION).unwrap();
        assert_eq!(agg, expected_aggregate, "nothing but the revision moved");
    }

    /// `the_refunding_split_spreads_one_set_across_two_positions` and its
    /// merge dual: the holder never touches the failure coordinate, the escrow
    /// touches nothing else, and the aggregate cannot tell the two shapes
    /// apart.
    #[test]
    fn a_refunding_mint_seats_the_failure_coordinate_in_the_escrow_and_the_merge_burns_it_there()
    {
        let mut agg = aggregate(&[10; 4], 0);
        let mut holder = position(0x21, &[0; 4], 0);
        let mut escrow = position(0x31, &[0, 0, 0, 10], 0);
        let post = apply_complete_set_v1(
            &mut agg,
            &mut holder,
            Some(&mut escrow),
            CompleteSetActV1 {
                direction: CompleteSetDirectionV1::Mint,
                quantity: 5,
                refunding: true,
            },
        )
        .unwrap();
        assert_eq!(post.failure_selector, Some(3));
        assert_eq!(supplies(&agg), vec![15; 4]);
        assert_eq!(balances(&holder), vec![5, 5, 5, 0]);
        assert_eq!(balances(&escrow), vec![0, 0, 0, 15]);
        assert_eq!(post.escrow_revision, Some(1));
        // Seated: the escrow's failure balance is the whole failure supply.
        assert_eq!(balances(&escrow)[3], supplies(&agg)[3]);
        apply_complete_set_v1(
            &mut agg,
            &mut holder,
            Some(&mut escrow),
            CompleteSetActV1 {
                direction: CompleteSetDirectionV1::Merge,
                quantity: 5,
                refunding: true,
            },
        )
        .unwrap();
        assert_eq!(supplies(&agg), vec![10; 4]);
        assert_eq!(balances(&holder), vec![0; 4]);
        assert_eq!(balances(&escrow), vec![0, 0, 0, 10]);
    }

    /// `a_holder_without_the_failure_coordinate_cannot_merge`, and its
    /// categorical sibling: a merge of more than is held at any coordinate is
    /// `Holding`, named, and the failure coordinate counts on a categorical
    /// Market.
    #[test]
    fn a_merge_of_an_incomplete_set_refuses_holding() {
        let mut agg = aggregate(&[10; 4], 0);
        let mut holder = position(0x21, &[5, 5, 4, 5], 0);
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut holder,
                None,
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Merge,
                    quantity: 5,
                    refunding: false,
                },
            ),
            Err(CompleteSetErrorV1::Holding),
        );
        // A refunding holder of the ordinary coordinates only, asked to merge
        // CATEGORICALLY, lacks the failure coordinate and refuses by the same
        // name -- the foreclosure the refunding law exists because of.
        let mut holder = position(0x21, &[5, 5, 5, 0], 0);
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut holder,
                None,
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Merge,
                    quantity: 5,
                    refunding: false,
                },
            ),
            Err(CompleteSetErrorV1::Holding),
        );
    }

    #[test]
    fn the_escrow_is_required_exactly_when_the_record_refunds() {
        let mut agg = aggregate(&[10; 4], 0);
        let mut holder = position(0x21, &[0; 4], 0);
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut holder,
                None,
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Mint,
                    quantity: 1,
                    refunding: true,
                },
            ),
            Err(CompleteSetErrorV1::EscrowRequired),
        );
        let mut escrow = position(0x31, &[0; 4], 0);
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut holder,
                Some(&mut escrow),
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Mint,
                    quantity: 1,
                    refunding: false,
                },
            ),
            Err(CompleteSetErrorV1::EscrowForbidden),
        );
    }

    #[test]
    fn a_zero_quantity_and_an_overflowing_credit_refuse_by_name() {
        let mut agg = aggregate(&[10; 4], 0);
        let mut holder = position(0x21, &[0; 4], 0);
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut holder,
                None,
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Mint,
                    quantity: 0,
                    refunding: false,
                },
            ),
            Err(CompleteSetErrorV1::Quantity),
        );
        let mut agg = aggregate(&[u64::MAX - 1; 4], 0);
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut holder,
                None,
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Mint,
                    quantity: 2,
                    refunding: false,
                },
            ),
            Err(CompleteSetErrorV1::Overflow),
        );
    }

    #[test]
    fn a_position_of_another_basis_or_width_does_not_join() {
        let mut agg = aggregate(&[10; 4], 0);
        let mut bytes = vec![
            0_u8;
            liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, WIDTH)
                .unwrap()
        ];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 0,
                market_account: [0x0a; 32],
                owner: [0x21; 32],
                basis_id: [0x16; 32],
            },
            &[0; 4],
            &mut bytes,
        )
        .unwrap();
        assert_eq!(
            apply_complete_set_v1(
                &mut agg,
                &mut bytes,
                None,
                CompleteSetActV1 {
                    direction: CompleteSetDirectionV1::Mint,
                    quantity: 1,
                    refunding: false,
                },
            ),
            Err(CompleteSetErrorV1::Join),
        );
    }

    /// The vault-principal relation: backing is an inequality, a shortfall is
    /// the refusal, and a stranger's donation cannot brick the Market.
    #[test]
    fn the_vault_backs_the_outstanding_supply_or_the_act_refuses() {
        let agg = aggregate(&[10; 4], 0);
        let exact = principal_v1(&agg, 30, 3).unwrap();
        assert_eq!(
            exact,
            PrincipalV1 {
                outstanding_sets: 10,
                required_backing_atoms: 30,
                vault_atoms: 30,
            }
        );
        assert_eq!(
            principal_v1(&agg, 31, 3).unwrap().vault_atoms,
            31,
            "a donated atom is excess backing, not a refusal",
        );
        assert_eq!(
            principal_v1(&agg, 29, 3),
            Err(CompleteSetErrorV1::Backing),
        );
        assert_eq!(principal_v1(&agg, 30, 0), Err(CompleteSetErrorV1::Quantity));
        // Non-uniform supply (a Market past Terminal) backs its LARGEST
        // coordinate: that is the L4 statement, not an average.
        let uneven = aggregate(&[10, 4, 0, 10], 0);
        assert_eq!(principal_v1(&uneven, 30, 3).unwrap().outstanding_sets, 10);
        assert_eq!(
            principal_v1(&uneven, 12, 3),
            Err(CompleteSetErrorV1::Backing),
        );
    }

    #[test]
    fn held_complete_sets_skip_the_failure_coordinate_only_when_the_record_refunds() {
        let holder = position(0x21, &[5, 7, 6, 0], 0);
        assert_eq!(held_complete_sets_v1(&holder, true).unwrap(), 5);
        assert_eq!(held_complete_sets_v1(&holder, false).unwrap(), 0);
    }

    /// The scalar writer that `signed_delta_v3` and `affine_batch_v2` now
    /// share: the same read, the same checked movement, the same put, and both
    /// delta vocabularies convert onto it without loss.
    #[test]
    fn both_delta_vocabularies_convert_onto_the_one_writer() {
        use crate::affine_batch_v2::{DeltaDirectionV2, SignedMagnitudeV2};
        use crate::signed_delta_v3::{DeltaDirectionV3, SignedDeltaV3};
        assert_eq!(
            CoordinateDeltaV1::from(SignedDeltaV3::new(DeltaDirectionV3::Credit, 7).unwrap()),
            CoordinateDeltaV1::Credit(7),
        );
        assert_eq!(
            CoordinateDeltaV1::from(SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).unwrap()),
            CoordinateDeltaV1::Neutral,
        );
        assert_eq!(
            CoordinateDeltaV1::from(SignedMagnitudeV2::new(DeltaDirectionV2::Debit, 3).unwrap()),
            CoordinateDeltaV1::Debit(3),
        );
        let mut agg = aggregate(&[10; 4], 0);
        apply_coordinate_v1(
            &mut agg,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            2,
            CoordinateDeltaV1::Debit(3),
        )
        .unwrap();
        assert_eq!(supplies(&agg), vec![10, 10, 7, 10]);
        assert_eq!(
            apply_coordinate_v1(
                &mut agg,
                LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                4,
                CoordinateDeltaV1::Credit(1),
            ),
            Err(CompleteSetErrorV1::Coordinate),
            "a coordinate past the runtime tail does not fit the candidate",
        );
    }
}
