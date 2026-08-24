//! Exact account and Token-2022 contracts for implemented current actions.
//!
//! Implementation is not release admission. The six entries below let the
//! wrapper, base program, capability manifest, and client tooling share one
//! closed description. The default adapter remains disabled; the named
//! successor development profile admits exactly this set.

use crate::runtime_contract::StructuredClaimActionV1;

/// Canonical label committing the exact current action/count/token-effect set.
pub const STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1: &str =
    "dragons-clutch/structured-claim/current-account-contract/v1/a1=34:init-mint;a3=32:mint;a5=32:burn;a6=32:dispose-hoard-surplus;a7=33:burn;a8=33:revoke-mint-authority";
/// SHA-256 identity of [`STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1`].
pub const STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1: [u8; 32] = [
    0x34, 0x78, 0xc3, 0xac, 0x60, 0x65, 0xc2, 0x29, 0x16, 0xf4, 0x57, 0xb3, 0x6f, 0xf3, 0x6d, 0xbb,
    0x95, 0xac, 0xd5, 0x34, 0x46, 0xf8, 0x62, 0x4e, 0x37, 0x04, 0xaf, 0x45, 0x74, 0xac, 0x76, 0x9a,
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
pub const STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1: usize = 33;

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
        for withdrawn_tag in [2_u8, 4_u8] {
            assert_eq!(
                StructuredClaimActionV1::from_tag(withdrawn_tag),
                Err(crate::runtime_contract::Error::UnknownAction),
            );
            assert_eq!(
                IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1 & (1_u16 << withdrawn_tag),
                0,
            );
        }
    }
}
