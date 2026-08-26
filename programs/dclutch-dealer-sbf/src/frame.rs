//! Exact plan-derived child resources for the Dealer adapter.
//!
//! Account width is a consequence of the interpreted economic plan, not a
//! universal maximum frame. The outer adapter authenticates its small common
//! prefix, interprets the transition, then accepts exactly the Claims and
//! Custody resources named here.

use dclutch_dealer_codec::{ClaimAction, CustodyRole, Plan};

const CUSTODY_ROLE_COUNT: usize = 9;
const _: () = assert!(CUSTODY_ROLE_COUNT <= 16);

pub(crate) const DYNAMIC_CUSTODY_ROLES: [CustodyRole; 6] = [
    CustodyRole::TakerQuote,
    CustodyRole::Executor,
    CustodyRole::DealerOwner,
    CustodyRole::UnwindRecipient,
    CustodyRole::FeeRecipient,
    CustodyRole::MarketHoard,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameError {
    NonCanonicalPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChildResources {
    pub(crate) claims: bool,
    pub(crate) dealer_position: bool,
    pub(crate) actor_position: bool,
    pub(crate) custody_transfer_count: usize,
    custody_roles: u16,
}

impl ChildResources {
    pub(crate) fn derive(plan: Plan) -> Result<Self, FrameError> {
        let (claims, dealer_position, actor_position) = match plan.claim {
            ClaimAction::None => (false, false, false),
            ClaimAction::Transfer { .. } => (true, true, true),
            ClaimAction::Redeem { .. } => (true, true, false),
            ClaimAction::AdjustLiquidity { .. } => (true, true, true),
        };
        let mut custody_transfer_count = 0_usize;
        let mut custody_roles = 0_u16;
        let mut observed_absent = false;
        for transfer in plan.custody {
            match transfer {
                None => observed_absent = true,
                Some(transfer) => {
                    if observed_absent
                        || transfer.amount == 0
                        || transfer.source == transfer.destination
                    {
                        return Err(FrameError::NonCanonicalPlan);
                    }
                    custody_transfer_count = custody_transfer_count
                        .checked_add(1)
                        .ok_or(FrameError::NonCanonicalPlan)?;
                    custody_roles |= role_mask(transfer.source) | role_mask(transfer.destination);
                }
            }
        }
        Ok(Self {
            claims,
            dealer_position,
            actor_position,
            custody_transfer_count,
            custody_roles,
        })
    }

    pub(crate) const fn requires_custody(self) -> bool {
        self.custody_transfer_count != 0
    }

    pub(crate) const fn requires_custody_role(self, role: CustodyRole) -> bool {
        self.custody_roles & role_mask(role) != 0
    }

    pub(crate) const fn custody_role_count(self) -> usize {
        let mut remaining = self.custody_roles;
        let mut count = 0_usize;
        while remaining != 0 {
            if remaining & 1 == 1 {
                count += 1;
            }
            remaining >>= 1;
        }
        count
    }
}

const fn role_mask(role: CustodyRole) -> u16 {
    1_u16 << custody_role_slot(role)
}

pub(crate) const fn custody_role_slot(role: CustodyRole) -> usize {
    match role {
        CustodyRole::DealerQuote => 0,
        CustodyRole::TakerQuote => 1,
        CustodyRole::FeeVault => 2,
        CustodyRole::LivenessVault => 3,
        CustodyRole::Executor => 4,
        CustodyRole::DealerOwner => 5,
        CustodyRole::UnwindRecipient => 6,
        CustodyRole::FeeRecipient => 7,
        CustodyRole::MarketHoard => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_dealer_codec::{CustodyTransfer, Side};

    fn transfer(
        source: CustodyRole,
        destination: CustodyRole,
        amount: u64,
    ) -> Option<CustodyTransfer> {
        Some(CustodyTransfer {
            source,
            destination,
            amount,
        })
    }

    #[test]
    fn empty_plan_has_no_child_resources() {
        let resources = ChildResources::derive(Plan {
            claim: ClaimAction::None,
            custody: [None; 3],
        })
        .expect("empty canonical plan");
        assert!(!resources.claims);
        assert!(!resources.requires_custody());
        assert_eq!(resources.custody_transfer_count, 0);
        assert_eq!(resources.custody_role_count(), 0);
    }

    #[test]
    fn fill_frame_is_claims_plus_only_five_custody_accounts() {
        let resources = ChildResources::derive(Plan {
            claim: ClaimAction::Transfer {
                side: Side::TakerBuys,
                outcome: 1,
                quantity: 7,
            },
            custody: [
                transfer(CustodyRole::TakerQuote, CustodyRole::DealerQuote, 90),
                transfer(CustodyRole::TakerQuote, CustodyRole::FeeVault, 3),
                transfer(CustodyRole::LivenessVault, CustodyRole::Executor, 2),
            ],
        })
        .expect("canonical fill plan");
        assert!(resources.claims);
        assert!(resources.dealer_position);
        assert!(resources.actor_position);
        assert_eq!(resources.custody_transfer_count, 3);
        assert_eq!(resources.custody_role_count(), 5);
        for role in [
            CustodyRole::DealerQuote,
            CustodyRole::TakerQuote,
            CustodyRole::FeeVault,
            CustodyRole::LivenessVault,
            CustodyRole::Executor,
        ] {
            assert!(resources.requires_custody_role(role));
        }
        for role in [
            CustodyRole::DealerOwner,
            CustodyRole::UnwindRecipient,
            CustodyRole::FeeRecipient,
            CustodyRole::MarketHoard,
        ] {
            assert!(!resources.requires_custody_role(role));
        }
    }

    #[test]
    fn terminal_unwind_omits_actor_position_and_unrelated_vaults() {
        let resources = ChildResources::derive(Plan {
            claim: ClaimAction::Redeem {
                outcome: 0,
                quantity: 11,
                payout: 11,
            },
            custody: [
                transfer(CustodyRole::MarketHoard, CustodyRole::DealerQuote, 11),
                transfer(CustodyRole::LivenessVault, CustodyRole::Executor, 2),
                None,
            ],
        })
        .expect("canonical unwind plan");
        assert!(resources.claims);
        assert!(resources.dealer_position);
        assert!(!resources.actor_position);
        assert_eq!(resources.custody_transfer_count, 2);
        assert_eq!(resources.custody_role_count(), 4);
        assert!(resources.requires_custody_role(CustodyRole::MarketHoard));
        assert!(!resources.requires_custody_role(CustodyRole::TakerQuote));
        assert!(!resources.requires_custody_role(CustodyRole::FeeVault));
    }

    #[test]
    fn funded_replacement_keeps_custody_but_omits_claims() {
        let resources = ChildResources::derive(Plan {
            claim: ClaimAction::None,
            custody: [
                transfer(CustodyRole::LivenessVault, CustodyRole::DealerOwner, 5),
                transfer(CustodyRole::DealerOwner, CustodyRole::LivenessVault, 9),
                None,
            ],
        })
        .expect("canonical replacement funding plan");
        assert!(!resources.claims);
        assert!(resources.requires_custody());
        assert_eq!(resources.custody_transfer_count, 2);
        assert_eq!(resources.custody_role_count(), 2);
    }

    #[test]
    fn native_claim_liquidity_uses_both_positions_without_custody() {
        for add in [true, false] {
            let resources = ChildResources::derive(Plan {
                claim: ClaimAction::AdjustLiquidity {
                    add,
                    outcome: 2,
                    quantity: 13,
                },
                custody: [None; 3],
            })
            .expect("canonical native-claim liquidity plan");
            assert!(resources.claims);
            assert!(resources.dealer_position);
            assert!(resources.actor_position);
            assert!(!resources.requires_custody());
        }
    }

    #[test]
    fn holes_zero_amounts_and_reflexive_transfers_refuse() {
        let cases = [
            Plan {
                claim: ClaimAction::None,
                custody: [
                    None,
                    transfer(CustodyRole::DealerOwner, CustodyRole::LivenessVault, 1),
                    None,
                ],
            },
            Plan {
                claim: ClaimAction::None,
                custody: [
                    transfer(CustodyRole::DealerOwner, CustodyRole::LivenessVault, 0),
                    None,
                    None,
                ],
            },
            Plan {
                claim: ClaimAction::None,
                custody: [
                    transfer(CustodyRole::DealerOwner, CustodyRole::DealerOwner, 1),
                    None,
                    None,
                ],
            },
        ];
        for plan in cases {
            assert_eq!(
                ChildResources::derive(plan),
                Err(FrameError::NonCanonicalPlan)
            );
        }
    }
}
