//! Exact account and Token-2022 contracts for implemented current actions.
//!
//! Implementation is not release admission. The six entries below let the
//! wrapper, base program, capability manifest, and client tooling share one
//! closed description while the executable masks remain zero.

use crate::runtime_contract::StructuredClaimActionV1;

/// Canonical label committing the exact current action/count/token-effect set.
pub const STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1: &str =
    "dragons-clutch/structured-claim/current-account-contract/v1/a1=34:init-mint;a3=32:mint;a5=32:burn;a6=32:dispose-hoard-surplus;a7=33:burn;a8=31:revoke-mint-authority";
/// SHA-256 identity of [`STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1`].
pub const STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1: [u8; 32] = [
    0xe6, 0x56, 0x17, 0xe2, 0x8e, 0xdc, 0x5a, 0xcc, 0x8b, 0x47, 0x72, 0xf0, 0x3b, 0xc4, 0xef, 0xc8,
    0x41, 0xe9, 0x42, 0x81, 0x3c, 0x53, 0x26, 0x36, 0x9c, 0xcf, 0xe5, 0xc3, 0xc4, 0x12, 0xf6, 0x84,
];

/// Exact account count for action 1.
pub const STRUCTURED_CREATE_ACCOUNT_COUNT_V1: usize = 34;
/// Exact account count for actions 3 and 5.
pub const STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1: usize = 32;
/// Exact account count for action 6.
pub const STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1: usize = 32;
/// Exact account count for action 7.
pub const STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1: usize = 33;
/// Exact account count for action 8.
pub const STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1: usize = 31;

/// Implemented current action bits. This is not an executable capability mask.
pub const IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1: u16 =
    (1_u16 << StructuredClaimActionV1::CreateDescriptor.tag())
        | (1_u16 << StructuredClaimActionV1::WrapFull.tag())
        | (1_u16 << StructuredClaimActionV1::UnwrapFull.tag())
        | (1_u16 << StructuredClaimActionV1::CompactDonation.tag())
        | (1_u16 << StructuredClaimActionV1::RedeemTerminal.tag())
        | (1_u16 << StructuredClaimActionV1::RetireDescriptor.tag());

/// Token-side effect owned by one exact current action contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CurrentStructuredTokenEffectV1 {
    /// Create the permanent zero-supply wrapper mint under its exact PDA.
    InitializeWrapper = 1,
    /// Mint exact wrapper quantity after full backing enters custody.
    MintWrapper = 2,
    /// Burn exact wrapper quantity before backing leaves custody.
    BurnWrapper = 3,
    /// Optionally transfer only destroyed cash liability Hoard-to-neutral.
    DisposeHoardSurplus = 4,
    /// Revoke authority from the already zero-supply permanent mint.
    RevokeMintAuthority = 5,
}

/// One closed implemented action/account contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredActionContractV1 {
    /// Canonical family-local action.
    pub action: StructuredClaimActionV1,
    /// Exact outer and base account count.
    pub account_count: u8,
    /// Exact Token-2022-side effect.
    pub token_effect: CurrentStructuredTokenEffectV1,
}

/// Complete implemented current action set. Withdrawn canonical actions 2 and
/// 4 intentionally have no current account contract and cannot be admitted.
pub const CURRENT_STRUCTURED_ACTION_CONTRACTS_V1: [CurrentStructuredActionContractV1; 6] = [
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::CreateDescriptor,
        account_count: STRUCTURED_CREATE_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::InitializeWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::WrapFull,
        account_count: STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::MintWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::UnwrapFull,
        account_count: STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::BurnWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::CompactDonation,
        account_count: STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::DisposeHoardSurplus,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::RedeemTerminal,
        account_count: STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::BurnWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::RetireDescriptor,
        account_count: STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::RevokeMintAuthority,
    },
];

/// Return the sole implemented current contract for an action.
pub const fn current_structured_action_contract_v1(
    action: StructuredClaimActionV1,
) -> Option<CurrentStructuredActionContractV1> {
    match action {
        StructuredClaimActionV1::CreateDescriptor => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[0])
        }
        StructuredClaimActionV1::WrapFull => Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[1]),
        StructuredClaimActionV1::UnwrapFull => Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[2]),
        StructuredClaimActionV1::CompactDonation => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[3])
        }
        StructuredClaimActionV1::RedeemTerminal => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[4])
        }
        StructuredClaimActionV1::RetireDescriptor => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[5])
        }
        StructuredClaimActionV1::WrapCanonical
        | StructuredClaimActionV1::UnwrapCanonical => None,
    }
}

const _: () = assert!(STRUCTURED_CREATE_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1 <= u8::MAX as usize);

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn implemented_contracts_are_exact_and_withdrawn_routes_have_none() {
        let mut observed_mask = 0_u16;
        for contract in CURRENT_STRUCTURED_ACTION_CONTRACTS_V1 {
            observed_mask |= 1_u16 << contract.action.tag();
            assert_eq!(
                current_structured_action_contract_v1(contract.action),
                Some(contract)
            );
        }
        assert_eq!(observed_mask, IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1);
        assert_eq!(
            <[u8; 32]>::from(
                Sha256::digest(STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1.as_bytes())
            ),
            STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
        );
        for action in [
            StructuredClaimActionV1::WrapCanonical,
            StructuredClaimActionV1::UnwrapCanonical,
        ] {
            assert_eq!(current_structured_action_contract_v1(action), None);
            assert_eq!(
                IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1
                    & (1_u16 << action.tag()),
                0
            );
        }
    }
}
