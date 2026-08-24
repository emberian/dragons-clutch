//! Current hostile-observation and terminal-close projections.
//!
//! These fixed values are reconstructed from exact Solana accounts by the
//! adapter. They carry no standalone Market or supply transition authority.

use crate::{Amount, Error, Result, StructuredClaimDescriptorV2};

/// Canonical addresses derived by the SBF adapter from wrapper product identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct StructuredClaimRuntimeAddressesV1 {
    /// Immutable descriptor account.
    pub descriptor: [u8; 32],
    /// Extension-free Token-2022 wrapper mint.
    pub mint: [u8; 32],
    /// PDA holding mint authority.
    pub mint_authority: [u8; 32],
    /// Semantic owner of the base Position holding canonical backing.
    pub vault_owner: [u8; 32],
}

impl StructuredClaimRuntimeAddressesV1 {
    pub(crate) fn validate(&self) -> Result<()> {
        let keys = [
            self.descriptor,
            self.mint,
            self.mint_authority,
            self.vault_owner,
        ];
        let mut left = 0_usize;
        while left < keys.len() {
            if keys[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < keys.len() {
                if keys[left] == keys[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }
}

/// Hostile-decoded extension-free Token-2022 wrapper-mint observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperMintProjectionV1 {
    /// Canonical mint account.
    pub address: [u8; 32],
    /// Canonical wrapper authority PDA, or zero only after exact revocation.
    pub mint_authority: [u8; 32],
    /// Actual Token-2022 supply.
    pub supply: Amount,
    /// Must be zero for indivisible wrapper atoms.
    pub decimals: u8,
    /// Must be absent, encoded as zero.
    pub freeze_authority: [u8; 32],
    /// Must be zero: no mint extension is admitted by version one.
    pub extension_mask: u64,
    /// Initialized mint bit from the canonical Token-2022 parser.
    pub initialized: bool,
}

/// Hostile-decoded wrapper-token account observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperTokenProjectionV1 {
    /// Token account address.
    pub address: [u8; 32],
    /// Canonical wrapper mint.
    pub mint: [u8; 32],
    /// Bearer authority for the current instruction.
    pub owner: [u8; 32],
    /// Actual token amount.
    pub amount: Amount,
    /// Initialized account bit from the canonical Token-2022 parser.
    pub initialized: bool,
}

/// Base-program Position retirement capability authenticated by the SBF
/// adapter from the exact successor close plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVaultRetirementV1 {
    /// Content identity of the complete base close plan.
    pub close_receipt: [u8; 32],
    /// Market bound by that plan.
    pub market: [u8; 32],
    /// Wrapper vault's semantic Position owner.
    pub vault_owner: [u8; 32],
    /// Exact canonical Position V3 account being shrunk to its tombstone.
    pub position_account: [u8; 32],
    /// Exact purpose-owned Replay V3 account being deleted.
    pub replay_account: [u8; 32],
    /// Exact Position generation being closed.
    pub generation: u64,
    /// Exact Replay sequence consumed by close.
    pub replay_sequence: u64,
    /// Permanent base tombstone produced by close.
    pub tombstone: [u8; 32],
    /// Semantic identity of the terminal Replay V3 prefix and extension.
    pub terminal_replay_semantic_id: [u8; 32],
    /// Exact rent transition admitted when this Position/Replay pair was founded.
    pub rent_transition_id: [u8; 32],
    /// Persisted payer receiving only refundable live principal.
    pub rent_refund_owner: [u8; 32],
    /// Realm-selected sink receiving every lamport that is not principal.
    pub neutral_lamport_sink: [u8; 32],
    /// Principal retained permanently in the Position V3 tombstone.
    pub position_tombstone_principal_lamports: u64,
    /// Refundable Position live principal returned to the persisted payer.
    pub position_refund_lamports: u64,
    /// Refundable Replay principal returned to the persisted payer.
    pub replay_refund_lamports: u64,
    /// Position surplus sent only to the neutral sink.
    pub position_donation_lamports: u64,
    /// Replay surplus sent only to the neutral sink.
    pub replay_donation_lamports: u64,
}

/// Permanent descriptor, mint, and canonical vault retirement projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DescriptorRetirementPlanV1 {
    /// Prospective permanent descriptor image.
    pub descriptor: StructuredClaimDescriptorV2,
    /// Actual mint supply, necessarily zero.
    pub mint_supply: Amount,
    /// Mint authority before Token-2022 SetAuthority.
    pub mint_authority_before: [u8; 32],
    /// Mint authority after revocation, encoded absent as zero.
    pub mint_authority_after: [u8; 32],
    /// Base close plan that must execute in the same atomic transaction.
    pub vault_close_receipt: [u8; 32],
    /// Permanent base Position tombstone.
    pub vault_tombstone: [u8; 32],
    /// Exact Position account rewritten to the permanent tombstone.
    pub vault_position_account: [u8; 32],
    /// Exact Replay account deleted atomically with the Position rewrite.
    pub vault_replay_account: [u8; 32],
    /// Immutable semantic owner of the Structured backing Position.
    pub vault_owner: [u8; 32],
    /// Semantic identity of the terminal purpose-owned Replay.
    pub terminal_replay_semantic_id: [u8; 32],
    /// Principal retained in the permanent Position tombstone.
    pub vault_tombstone_principal_lamports: u64,
    /// Total payer-owned principal refunded by the base close.
    pub vault_refund_lamports: u64,
    /// Total non-principal lamports sent to the neutral sink.
    pub vault_donation_lamports: u64,
    /// Persisted payer receiving the exact refund.
    pub rent_refund_owner: [u8; 32],
    /// Realm-selected beneficiary-free sink.
    pub neutral_lamport_sink: [u8; 32],
}
