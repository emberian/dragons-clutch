// SPDX-License-Identifier: AGPL-3.0-or-later
//! Falsifier tests for the TerminalIdentityV1 lifecycle. Each test states a
//! claim from `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md` §1 and tries to
//! break it through the public API; a pass means the attempt refused or the
//! exact conserved quantity held.

use clutch_terminal_identity_v1::{CloseSplit, Error, Id, LivenessError, TerminalAccountV1};

fn id(byte: u8) -> Id {
    Id::from_bytes([byte; 32])
}

fn sink() -> Id {
    id(250)
}

/// Claim: a prefund can never reach the payer. Any balance present before
/// the payer's exact deposit appears in `neutral_surplus` at close and never
/// in the payer's credit, with or without later donations.
#[test]
fn falsifier_prefund_can_never_reach_the_payer() {
    for prefund in 1u64..=64 {
        for shortfall in 1u64..=16 {
            let account =
                TerminalAccountV1::create(id(1), sink(), prefund, shortfall, prefund + shortfall)
                    .unwrap();
            let (_, split) = account.close(prefund + shortfall, shortfall).unwrap();
            assert_eq!(split.payer_principal, shortfall);
            assert_eq!(split.neutral_surplus, prefund);

            for donation in 0u64..=8 {
                let balance = prefund + shortfall + donation;
                let account = account.observe_transition(balance, shortfall).unwrap();
                let (_, split) = account.close(balance, shortfall).unwrap();
                assert_eq!(split.payer_principal, shortfall);
                assert_eq!(split.neutral_surplus, prefund + donation);
            }
        }
    }
}

/// Claim: donations mid-life accrete monotonically and never reduce the
/// recorded payer principal.
#[test]
fn falsifier_donations_accrete_monotonically_and_never_reduce_principal() {
    let mut account = TerminalAccountV1::create(id(1), sink(), 5, 17, 22).unwrap();
    let mut floor = account.header().donation_floor;
    assert_eq!(floor, 5);
    for donation in [0u64, 3, 0, 11, 1, 0, 40] {
        let balance = 17 + floor + donation;
        account = account.observe_transition(balance, 17).unwrap();
        assert!(account.header().donation_floor >= floor);
        assert_eq!(account.header().donation_floor, floor + donation);
        assert_eq!(account.header().payer_principal, 17);
        floor = account.header().donation_floor;
    }
    // An observation one lamport below the accreted floor refuses; the floor
    // never moves back down.
    assert_eq!(
        account.observe_transition(17 + floor - 1, 17),
        Err(Error::Liveness(LivenessError::AccountBalanceShortfall))
    );
    assert_eq!(account.header().donation_floor, floor);
}

/// Claim: close conserves exactly — `payer_principal + neutral_surplus`
/// equals the closing balance with no remainder, over an exhaustive small
/// grid of prefunds, shortfalls, and later donations.
#[test]
fn falsifier_close_conservation_is_exact() {
    for shortfall in 1u64..=8 {
        for prefund in 0u64..=8 {
            for donation in 0u64..=8 {
                let account = TerminalAccountV1::create(
                    id(1),
                    sink(),
                    prefund,
                    shortfall,
                    prefund + shortfall,
                )
                .unwrap();
                let balance = prefund + shortfall + donation;
                let account = account.observe_transition(balance, shortfall).unwrap();
                let (_, split) = account.close(balance, shortfall).unwrap();
                assert_eq!(
                    split,
                    CloseSplit {
                        payer: id(1),
                        payer_principal: shortfall,
                        neutral_surplus: prefund + donation,
                        sink: sink(),
                    }
                );
                assert_eq!(split.payer_principal + split.neutral_surplus, balance);
            }
        }
    }
}

/// Claim: a deficit refuses rather than clamps. Whenever the actual balance
/// is below `accounted + donation_floor`, both observation and close refuse
/// in the kernel, and the refused copy is left able to close exactly at a
/// valid balance afterward.
#[test]
fn falsifier_deficit_refuses_rather_than_clamps() {
    let account = TerminalAccountV1::create(id(1), sink(), 5, 17, 22).unwrap();
    let account = account.observe_transition(30, 17).unwrap();
    let floor = account.header().donation_floor;
    assert_eq!(floor, 13);
    for deficit in 1u64..=30 {
        let short_balance = (17 + floor).saturating_sub(deficit);
        assert_eq!(
            account.observe_transition(short_balance, 17),
            Err(Error::Liveness(LivenessError::AccountBalanceShortfall))
        );
        assert_eq!(
            account.close(short_balance, 17),
            Err(Error::Liveness(LivenessError::AccountBalanceShortfall))
        );
    }
    // No clamping happened: the same value still closes exactly.
    let (_, split) = account.close(30, 17).unwrap();
    assert_eq!(split.payer_principal, 17);
    assert_eq!(split.neutral_surplus, 13);
}
