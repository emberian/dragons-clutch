//! Hostiles for the governed record.
//!
//! Each names its exact discriminant. A bare `is_err()` here would accept the
//! length refusal, the header refusal and every band conjunct, which is the
//! failure mode `AGENTS.md` measured nineteen times in one week.

use super::*;

const AUTHORITY: [u8; 32] = [7; 32];
const STRANGER_BENEFICIARY: [u8; 32] = [9; 32];
/// The rent-exempt minimum of a 288-byte claim check at the kernel's reference
/// rate: `(128 + 288) * 3480 * 2`. The funded-crank floor, chain-derived rather
/// than chosen (`docs/design/FUNDED_CRANK_V1.md` section 3).
const FUNDED_CRANK_FLOOR: u64 = 2_895_360;
const NOW: u64 = 500_000_000;

fn genesis_record() -> ProtocolParametersRecordV1 {
    ProtocolParametersRecordV1 {
        bump: 255,
        parameters: ProtocolParametersV1::genesis(AUTHORITY),
        pending: PendingChangeV1::NONE,
    }
}

/// The record is born holding exactly today's deployed economics.
#[test]
fn genesis_is_todays_numbers_and_is_in_band() {
    let genesis = ProtocolParametersV1::genesis(AUTHORITY);
    assert!(genesis.in_band());
    // Decision 0014 D2, and the value the Direct codec enforces today.
    assert_eq!(genesis.max_fee_basis_points, 500);
    // Ruling D1 item 1: no protocol take, no protocol beneficiary.
    assert_eq!(genesis.protocol_take_basis_points, 0);
    assert_eq!(genesis.protocol_beneficiary, [0; 32]);
    // `COMPACTION_CRANK_REWARD_LAMPORTS_V1` as it stands in `claim_check_v1.rs`.
    assert_eq!(genesis.crank_reward_cap_lamports, 200_000);
    // `DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1 = 0`, reached by the CAP: the share
    // is already the whole donation slice, so the one number that has to move
    // to pay a closer is the cap.
    assert_eq!(genesis.closer_reward_cap_lamports, 0);
    assert_eq!(genesis.closer_carve_basis_points, 10_000);
    assert_eq!(genesis.closer_carve(1_000_000_000), 0);
}

#[test]
fn a_record_round_trips_and_refuses_reserved_bytes_and_a_short_wire() {
    let record = genesis_record();
    let bytes = record.to_bytes();
    assert_eq!(ProtocolParametersRecordV1::decode(&bytes), Ok(record));
    assert_eq!(
        ProtocolParametersRecordV1::decode(
            bytes
                .get(..bytes.len() - 1)
                .expect("one byte short of the record"),
        ),
        Err(Error::InvalidLength),
    );
    for offset in [
        PROTOCOL_PARAMETERS_RECORD_RESERVED_HEADER_OFFSET,
        PROTOCOL_PARAMETERS_RECORD_RESERVED_TAIL_OFFSET,
    ] {
        let mut hostile = bytes;
        *hostile
            .get_mut(offset)
            .expect("a reserved offset inside the record") = 1;
        assert_eq!(
            ProtocolParametersRecordV1::decode(&hostile),
            Err(Error::NonCanonical),
        );
    }
    let mut wrong_kind = bytes;
    wrong_kind[PROTOCOL_PARAMETERS_RECORD_KIND_OFFSET] = PROTOCOL_PARAMETERS_RECORD_KIND_V1 + 1;
    assert_eq!(
        ProtocolParametersRecordV1::decode(&wrong_kind),
        Err(Error::InvalidHeader),
    );
}

/// HOSTILE 1: an unauthorized change refuses.
#[test]
fn an_unauthorized_change_refuses() {
    let record = genesis_record();
    let proposed = ProtocolParametersV1 {
        crank_reward_cap_lamports: 1,
        ..record.parameters
    };
    assert_eq!(
        record.propose(false, proposed, NOW),
        Err(Error::UnauthorizedGovernance),
    );
    // The control: the same proposal from the authority stages.
    assert!(record.propose(true, proposed, NOW).is_ok());
}

/// HOSTILE 2: a change inside the delay does not apply.
#[test]
fn a_change_inside_the_delay_does_not_apply() {
    let record = genesis_record();
    let proposed = ProtocolParametersV1 {
        closer_reward_cap_lamports: FUNDED_CRANK_FLOOR,
        ..record.parameters
    };
    let staged = record.propose(true, proposed, NOW).expect("stages");
    let matures = NOW + PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1;
    assert_eq!(staged.pending.earliest_apply_slot, matures);

    assert_eq!(
        staged.apply_change(proposed, matures - 1),
        Err(Error::ProposalNotMatured),
    );
    // One slot later, the same call succeeds. Without this the refusal above
    // could be any of four conjuncts.
    let (after, receipt) = staged.apply_change(proposed, matures).expect("applies");
    assert_eq!(
        after.parameters.closer_reward_cap_lamports,
        FUNDED_CRANK_FLOOR
    );
    assert_eq!(after.parameters.generation, 1);
    assert_eq!(after.parameters.activation_slot, matures);
    assert_eq!(after.pending, PendingChangeV1::NONE);

    // The receipt's own three slot numbers close, so a census reading only the
    // receipt can check the notice period.
    assert_eq!(receipt.proposed_at_slot, NOW);
    assert_eq!(receipt.delay_slots, PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1);
    assert_eq!(receipt.activation_slot, matures);
    assert_eq!(receipt.generation, 1);
    assert_ne!(receipt.previous_digest, receipt.new_digest);
    let encoded = receipt.to_bytes();
    assert_eq!(
        ProtocolParametersChangeReceiptV1::decode(&encoded),
        Ok(receipt),
    );
}

/// HOSTILE 3: a parameter outside its band refuses -- one band at a time, each
/// by the exact discriminant, and each with a control that admits.
#[test]
fn a_parameter_outside_its_band_refuses() {
    let record = genesis_record();
    let base = record.parameters;

    // The fee ceiling only narrows.
    assert_eq!(
        record.propose(
            true,
            ProtocolParametersV1 {
                max_fee_basis_points: PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1 + 1,
                ..base
            },
            NOW,
        ),
        Err(Error::ParameterOutOfBand),
    );
    assert!(
        record
            .propose(
                true,
                ProtocolParametersV1 {
                    max_fee_basis_points: PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1 - 200,
                    ..base
                },
                NOW,
            )
            .is_ok()
    );

    // A take with no payee, and a payee with no take: both unrepresentable.
    assert_eq!(
        record.propose(
            true,
            ProtocolParametersV1 {
                protocol_take_basis_points: 1,
                ..base
            },
            NOW,
        ),
        Err(Error::ParameterOutOfBand),
    );
    assert_eq!(
        record.propose(
            true,
            ProtocolParametersV1 {
                protocol_beneficiary: STRANGER_BENEFICIARY,
                ..base
            },
            NOW,
        ),
        Err(Error::ParameterOutOfBand),
    );
    // Together they admit, which is what makes the two refusals above a PAIR
    // rule rather than a prohibition on either field.
    assert!(
        record
            .propose(
                true,
                ProtocolParametersV1 {
                    protocol_take_basis_points: 25,
                    protocol_beneficiary: STRANGER_BENEFICIARY,
                    ..base
                },
                NOW,
            )
            .is_ok()
    );
    // And a take above the fee band it is taken out of refuses even with a payee.
    assert_eq!(
        record.propose(
            true,
            ProtocolParametersV1 {
                protocol_take_basis_points: base.max_fee_basis_points + 1,
                protocol_beneficiary: STRANGER_BENEFICIARY,
                ..base
            },
            NOW,
        ),
        Err(Error::ParameterOutOfBand),
    );

    // The carve is a share, so it cannot exceed the whole donation.
    assert_eq!(
        record.propose(
            true,
            ProtocolParametersV1 {
                closer_carve_basis_points: PROTOCOL_BASIS_POINT_DENOMINATOR_V1 + 1,
                ..base
            },
            NOW,
        ),
        Err(Error::ParameterOutOfBand),
    );

    // Governance cannot make itself instantaneous by governing its own delay.
    assert_eq!(
        record.propose(
            true,
            ProtocolParametersV1 {
                change_delay_slots: PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1 - 1,
                ..base
            },
            NOW,
        ),
        Err(Error::ParameterOutOfBand),
    );
    assert!(
        record
            .propose(
                true,
                ProtocolParametersV1 {
                    change_delay_slots: PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1 * 4,
                    ..base
                },
                NOW,
            )
            .is_ok()
    );
}

/// A proposal is a commitment to a VALUE, never a permission to install one.
#[test]
fn applying_bytes_other_than_the_ones_proposed_refuses() {
    let record = genesis_record();
    let proposed = ProtocolParametersV1 {
        closer_reward_cap_lamports: FUNDED_CRANK_FLOOR,
        ..record.parameters
    };
    let staged = record.propose(true, proposed, NOW).expect("stages");
    let matures = staged.pending.earliest_apply_slot;
    let substituted = ProtocolParametersV1 {
        closer_reward_cap_lamports: FUNDED_CRANK_FLOOR * 1_000,
        ..record.parameters
    };
    assert_eq!(
        staged.apply_change(substituted, matures),
        Err(Error::ProposalDigestMismatch),
    );
    assert!(staged.apply_change(proposed, matures).is_ok());
}

#[test]
fn one_proposal_stands_at_a_time_and_only_the_authority_withdraws_it() {
    let record = genesis_record();
    let first = ProtocolParametersV1 {
        crank_reward_cap_lamports: 1,
        ..record.parameters
    };
    let second = ProtocolParametersV1 {
        crank_reward_cap_lamports: 2,
        ..record.parameters
    };
    let staged = record.propose(true, first, NOW).expect("stages");
    assert_eq!(
        staged.propose(true, second, NOW),
        Err(Error::ProposalOutstanding),
    );
    assert_eq!(staged.withdraw(false), Err(Error::UnauthorizedGovernance));
    let withdrawn = staged.withdraw(true).expect("withdraws");
    assert_eq!(withdrawn.pending, PendingChangeV1::NONE);
    assert_eq!(withdrawn.withdraw(true), Err(Error::NoPendingProposal));
    assert!(withdrawn.propose(true, second, NOW).is_ok());
    assert_eq!(
        withdrawn.apply_change(second, NOW),
        Err(Error::NoPendingProposal),
    );
}

/// The one-way door: a zero authority refuses every proposal, forever.
#[test]
fn a_frozen_record_refuses_every_proposal_from_everybody() {
    let frozen = ProtocolParametersRecordV1 {
        parameters: ProtocolParametersV1 {
            governance_authority: [0; 32],
            ..ProtocolParametersV1::genesis(AUTHORITY)
        },
        ..genesis_record()
    };
    let proposed = ProtocolParametersV1 {
        crank_reward_cap_lamports: 1,
        ..frozen.parameters
    };
    for signer_is_authority in [false, true] {
        assert_eq!(
            frozen.propose(signer_is_authority, proposed, NOW),
            Err(Error::GovernanceFrozen),
        );
    }
    assert_eq!(frozen.withdraw(true), Err(Error::GovernanceFrozen));
    // It still round-trips and still reads: frozen is a governance state, not a
    // broken record.
    assert_eq!(
        ProtocolParametersRecordV1::decode(&frozen.to_bytes()),
        Ok(frozen),
    );
}

/// The carve is a share of the donation slice, then a cap, and never touches
/// principal -- the caller subtracts it from the donation, not from the total.
#[test]
fn the_closer_carve_is_a_capped_share_that_rounds_toward_the_beneficiary() {
    let half = ProtocolParametersV1 {
        closer_carve_basis_points: 5_000,
        closer_reward_cap_lamports: FUNDED_CRANK_FLOOR,
        ..ProtocolParametersV1::genesis(AUTHORITY)
    };
    assert!(half.in_band());
    // The share binds below the cap.
    assert_eq!(half.closer_carve(1_000_000), 500_000);
    // The cap binds above it: a donation of one SOL does not pay one SOL.
    assert_eq!(half.closer_carve(1_000_000_000), FUNDED_CRANK_FLOOR);
    // Rounding is toward the beneficiary, the same direction the Direct fee
    // rounds: half of an odd lamport count is the floor.
    assert_eq!(half.closer_carve(3), 1);
    // And it never refuses: a donation of nothing pays nothing.
    assert_eq!(half.closer_carve(0), 0);
    assert_eq!(half.closer_carve(1), 0);
}

/// A persisted record out of band refuses on the READ, so no consumer has to
/// re-ask the question every writer already answered.
#[test]
fn a_corrupted_record_that_widens_the_fee_band_refuses_on_decode() {
    let mut bytes = genesis_record().to_bytes();
    bytes[PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET
        ..PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET + 2]
        .copy_from_slice(&(PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1 + 1).to_le_bytes());
    assert_eq!(
        ProtocolParametersRecordV1::decode(&bytes),
        Err(Error::ParameterOutOfBand),
    );
    // Two bytes back, and it reads.
    bytes[PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET
        ..PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET + 2]
        .copy_from_slice(&PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1.to_le_bytes());
    assert!(ProtocolParametersRecordV1::decode(&bytes).is_ok());
}

/// A receipt whose own slot arithmetic does not close is not evidence of a
/// governed change.
#[test]
fn a_receipt_claiming_less_than_the_minimum_notice_refuses() {
    let record = genesis_record();
    let proposed = ProtocolParametersV1 {
        crank_reward_cap_lamports: 1,
        ..record.parameters
    };
    let staged = record.propose(true, proposed, NOW).expect("stages");
    let (_, receipt) = staged
        .apply_change(proposed, staged.pending.earliest_apply_slot)
        .expect("applies");
    let honest = receipt.to_bytes();
    assert!(ProtocolParametersChangeReceiptV1::decode(&honest).is_ok());

    let shortened = ProtocolParametersChangeReceiptV1 {
        delay_slots: PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1 - 1,
        ..receipt
    };
    assert_eq!(
        ProtocolParametersChangeReceiptV1::decode(&shortened.to_bytes()),
        Err(Error::NonCanonical),
    );
    // And one that claims a maturity later than the slot it says it activated.
    let backdated = ProtocolParametersChangeReceiptV1 {
        activation_slot: receipt.proposed_at_slot,
        ..receipt
    };
    assert_eq!(
        ProtocolParametersChangeReceiptV1::decode(&backdated.to_bytes()),
        Err(Error::NonCanonical),
    );
}

/// The derivation the delay floor rests on, checked rather than asserted in a
/// comment: `COMPACTION_DEADLINE_SLOTS_V1` is a hundred and eighty of these.
#[test]
fn the_nominal_day_is_the_one_the_compaction_deadline_already_assumed() {
    assert_eq!(180 * PROTOCOL_SLOTS_PER_NOMINAL_DAY_V1, 38_880_000);
    assert_eq!(
        PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1,
        7 * PROTOCOL_SLOTS_PER_NOMINAL_DAY_V1
    );
}
