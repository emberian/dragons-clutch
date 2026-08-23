// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::Identity32V1;

use crate::codec::{exact, identity, put_identity, put_u64, require_zeroes, u64_at};
use crate::{Error, Result};

pub use clutch_solana_layout::registry::FractionalRedemptionAction as FractionalRedemptionActionV1;

/// Fractional-redemption successor intent family.
pub const FRACTIONAL_REDEMPTION_FAMILY_TAG: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_FAMILY_TAG;
/// Fractional-redemption successor intent-family version.
pub const FRACTIONAL_REDEMPTION_FAMILY_VERSION: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_FAMILY_VERSION;

/// Exact initialization payload width.
pub const FRACTIONAL_INITIALIZE_INTENT_BYTES: usize = 24;
/// Exact internal/bearer redemption payload width.
pub const FRACTIONAL_REDEEM_INTENT_BYTES: usize = 168;
/// Exact credit transfer/merge payload width.
pub const FRACTIONAL_TRANSFER_INTENT_BYTES: usize = 208;
/// Exact zero-credit close payload width.
pub const FRACTIONAL_CLOSE_CREDIT_INTENT_BYTES: usize = 80;
/// Exact terminal seal/ledger close payload width.
pub const FRACTIONAL_TERMINAL_INTENT_BYTES: usize = 8;

/// Immutable policy/ledger initialization selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalInitializeIntentV1 {
    /// Resolution/credit-accounting generation.
    pub domain_generation: u64,
    /// Adapter-recomputed minimal common lot asserted by the client.
    pub common_lot: u64,
    /// Canonical policy PDA bump.
    pub policy_bump: u8,
    /// Canonical ledger PDA bump.
    pub ledger_bump: u8,
}

impl FractionalInitializeIntentV1 {
    /// Decode canonical action-owned bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_INITIALIZE_INTENT_BYTES)?;
        require_zeroes(input, 18, 24)?;
        let value = Self {
            domain_generation: u64_at(input, 0)?,
            common_lot: u64_at(input, 8)?,
            policy_bump: input[16],
            ledger_bump: input[17],
        };
        if value.domain_generation == 0 || value.common_lot == 0 {
            return Err(Error::InvalidPayout);
        }
        Ok(value)
    }

    /// Encode canonical action-owned bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_INITIALIZE_INTENT_BYTES]> {
        if self.domain_generation == 0 || self.common_lot == 0 {
            return Err(Error::InvalidPayout);
        }
        let mut output = [0u8; FRACTIONAL_INITIALIZE_INTENT_BYTES];
        put_u64(&mut output, 0, self.domain_generation)?;
        put_u64(&mut output, 8, self.common_lot)?;
        output[16] = self.policy_bump;
        output[17] = self.ledger_bump;
        Ok(output)
    }
}

/// Shared exact redemption selector for actions 2 through 5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRedeemIntentV1 {
    /// Aggregate ledger sequence consumed by this action.
    pub expected_ledger_sequence: u64,
    /// Credit sequence; zero only for exact-lot actions.
    pub expected_credit_sequence: u64,
    /// Canonical Position Replay V3 sequence; zero only for bearer actions.
    pub expected_position_replay_sequence: u64,
    /// Raw internal or bearer claim quantity.
    pub quantity: u64,
    /// Exact claimant owner.
    pub claimant: Identity32V1,
    /// Position V3 or bearer token-account identity.
    pub claim_source: Identity32V1,
    /// Position V3 or collateral-token payout destination.
    pub payout_target: Identity32V1,
    /// Owner credit PDA; exact-lot actions require the policy PDA as a
    /// non-optional sentinel and never read or write it as credit state.
    pub credit_or_policy: Identity32V1,
    /// Native outcome index.
    pub outcome: u8,
    /// Fresh credit, live credit, or tombstone reopen selector.
    pub credit_mode: u8,
}

impl FractionalRedeemIntentV1 {
    /// Decode canonical action-owned bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_REDEEM_INTENT_BYTES)?;
        require_zeroes(input, 162, 168)?;
        let value = Self {
            expected_ledger_sequence: u64_at(input, 0)?,
            expected_credit_sequence: u64_at(input, 8)?,
            expected_position_replay_sequence: u64_at(input, 16)?,
            quantity: u64_at(input, 24)?,
            claimant: identity(input, 32)?,
            claim_source: identity(input, 64)?,
            payout_target: identity(input, 96)?,
            credit_or_policy: identity(input, 128)?,
            outcome: input[160],
            credit_mode: input[161],
        };
        if value.expected_ledger_sequence == 0 || value.quantity == 0 || value.credit_mode > 3 {
            return Err(Error::ReplayMismatch);
        }
        Ok(value)
    }

    /// Encode canonical action-owned bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_REDEEM_INTENT_BYTES]> {
        if self.expected_ledger_sequence == 0 || self.quantity == 0 || self.credit_mode > 3 {
            return Err(Error::ReplayMismatch);
        }
        let mut output = [0u8; FRACTIONAL_REDEEM_INTENT_BYTES];
        put_u64(&mut output, 0, self.expected_ledger_sequence)?;
        put_u64(&mut output, 8, self.expected_credit_sequence)?;
        put_u64(&mut output, 16, self.expected_position_replay_sequence)?;
        put_u64(&mut output, 24, self.quantity)?;
        for (offset, value) in [
            (32, self.claimant),
            (64, self.claim_source),
            (96, self.payout_target),
            (128, self.credit_or_policy),
        ] {
            put_identity(&mut output, offset, value)?;
        }
        output[160] = self.outcome;
        output[161] = self.credit_mode;
        Ok(output)
    }
}

/// Exact same-domain credit transfer/merge selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTransferIntentV1 {
    /// Aggregate ledger sequence.
    pub expected_ledger_sequence: u64,
    /// Source credit sequence.
    pub expected_source_sequence: u64,
    /// Destination credit sequence.
    pub expected_destination_sequence: u64,
    /// Destination Position Replay sequence; zero for external payout.
    pub expected_payout_replay_sequence: u64,
    /// Explicit numerator; merge requires zero and derives the full source.
    pub numerator: u64,
    /// Source claimant.
    pub source_claimant: Identity32V1,
    /// Destination claimant.
    pub destination_claimant: Identity32V1,
    /// Source credit PDA.
    pub source_credit: Identity32V1,
    /// Destination credit/tombstone PDA.
    pub destination_credit: Identity32V1,
    /// Position V3 or Realm-collateral payout destination.
    pub payout_target: Identity32V1,
    /// Internal Position or external collateral target selector.
    pub payout_kind: u8,
    /// Live, fresh, or tombstone-backed destination selector.
    pub destination_mode: u8,
}

impl FractionalTransferIntentV1 {
    /// Decode canonical action-owned bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_TRANSFER_INTENT_BYTES)?;
        require_zeroes(input, 202, 208)?;
        let value = Self {
            expected_ledger_sequence: u64_at(input, 0)?,
            expected_source_sequence: u64_at(input, 8)?,
            expected_destination_sequence: u64_at(input, 16)?,
            expected_payout_replay_sequence: u64_at(input, 24)?,
            numerator: u64_at(input, 32)?,
            source_claimant: identity(input, 40)?,
            destination_claimant: identity(input, 72)?,
            source_credit: identity(input, 104)?,
            destination_credit: identity(input, 136)?,
            payout_target: identity(input, 168)?,
            payout_kind: input[200],
            destination_mode: input[201],
        };
        if value.expected_ledger_sequence == 0
            || value.expected_source_sequence == 0
            || value.expected_destination_sequence == 0
            || !(1..=2).contains(&value.payout_kind)
            || !(1..=3).contains(&value.destination_mode)
            || value.source_claimant == value.destination_claimant
            || value.source_credit == value.destination_credit
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(value)
    }

    /// Encode canonical action-owned bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_TRANSFER_INTENT_BYTES]> {
        if self.expected_ledger_sequence == 0
            || self.expected_source_sequence == 0
            || self.expected_destination_sequence == 0
            || !(1..=2).contains(&self.payout_kind)
            || !(1..=3).contains(&self.destination_mode)
            || self.source_claimant == self.destination_claimant
            || self.source_credit == self.destination_credit
        {
            return Err(Error::MismatchedBinding);
        }
        let mut output = [0u8; FRACTIONAL_TRANSFER_INTENT_BYTES];
        put_u64(&mut output, 0, self.expected_ledger_sequence)?;
        put_u64(&mut output, 8, self.expected_source_sequence)?;
        put_u64(&mut output, 16, self.expected_destination_sequence)?;
        put_u64(&mut output, 24, self.expected_payout_replay_sequence)?;
        put_u64(&mut output, 32, self.numerator)?;
        for (offset, value) in [
            (40, self.source_claimant),
            (72, self.destination_claimant),
            (104, self.source_credit),
            (136, self.destination_credit),
            (168, self.payout_target),
        ] {
            put_identity(&mut output, offset, value)?;
        }
        output[200] = self.payout_kind;
        output[201] = self.destination_mode;
        Ok(output)
    }
}

/// Exact zero-credit close selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCloseCreditIntentV1 {
    /// Aggregate ledger sequence.
    pub expected_ledger_sequence: u64,
    /// Owner-credit sequence.
    pub expected_credit_sequence: u64,
    /// Exact claimant signer.
    pub claimant: Identity32V1,
    /// Exact live credit PDA.
    pub credit_account: Identity32V1,
}

impl FractionalCloseCreditIntentV1 {
    /// Decode canonical action-owned bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_CLOSE_CREDIT_INTENT_BYTES)?;
        let value = Self {
            expected_ledger_sequence: u64_at(input, 0)?,
            expected_credit_sequence: u64_at(input, 8)?,
            claimant: identity(input, 16)?,
            credit_account: identity(input, 48)?,
        };
        if value.expected_ledger_sequence == 0 || value.expected_credit_sequence == 0 {
            return Err(Error::ReplayMismatch);
        }
        Ok(value)
    }

    /// Encode canonical action-owned bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_CLOSE_CREDIT_INTENT_BYTES]> {
        if self.expected_ledger_sequence == 0 || self.expected_credit_sequence == 0 {
            return Err(Error::ReplayMismatch);
        }
        let mut output = [0u8; FRACTIONAL_CLOSE_CREDIT_INTENT_BYTES];
        put_u64(&mut output, 0, self.expected_ledger_sequence)?;
        put_u64(&mut output, 8, self.expected_credit_sequence)?;
        put_identity(&mut output, 16, self.claimant)?;
        put_identity(&mut output, 48, self.credit_account)?;
        Ok(output)
    }
}

/// Exact terminal seal or ledger-close selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTerminalIntentV1 {
    /// Aggregate ledger sequence.
    pub expected_ledger_sequence: u64,
}

impl FractionalTerminalIntentV1 {
    /// Decode canonical action-owned bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_TERMINAL_INTENT_BYTES)?;
        let value = Self {
            expected_ledger_sequence: u64_at(input, 0)?,
        };
        if value.expected_ledger_sequence == 0 {
            return Err(Error::ReplayMismatch);
        }
        Ok(value)
    }

    /// Encode canonical action-owned bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_TERMINAL_INTENT_BYTES]> {
        if self.expected_ledger_sequence == 0 {
            return Err(Error::ReplayMismatch);
        }
        Ok(self.expected_ledger_sequence.to_le_bytes())
    }
}

/// Exact Solana meta geometry frozen for a future capability review.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalAccountContractV1 {
    /// Exact live-state account count, or fixed prefix width when suffixes are set.
    pub account_count: u8,
    /// Bit `i` requires account `i` to be writable.
    pub writable_mask: u32,
    /// Bit `i` requires account `i` to sign.
    pub signer_mask: u32,
    /// One read-only/writable mint meta per active outcome follows the prefix.
    pub outcome_mint_suffix: bool,
    /// Fixed roles following the outcome-mint suffix.
    pub post_mint_accounts: u8,
    /// Fresh/reopen mode appends payer and System after the fixed Rent role.
    pub credit_creation_suffix: bool,
}

/// Return the frozen account-count and mutability contract for one action.
///
/// Account-role names and order are documented in the crate README. Action 2's
/// complete adapter uses this geometry, but the central capability tuple stays
/// disabled until action 1 can create its canonical inputs.
pub const fn fractional_account_contract_v1(
    action: FractionalRedemptionActionV1,
) -> FractionalAccountContractV1 {
    match action {
        FractionalRedemptionActionV1::Initialize => FractionalAccountContractV1 {
            account_count: 13,
            writable_mask: 0b0_0001_1100_0001,
            signer_mask: 0b0_0000_0000_0001,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: false,
        },
        FractionalRedemptionActionV1::RedeemInternalExact => FractionalAccountContractV1 {
            account_count: 15,
            writable_mask: 0b111_0011_0000_0000,
            signer_mask: 0b000_0000_0000_0001,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: false,
        },
        FractionalRedemptionActionV1::RedeemBearerExact => FractionalAccountContractV1 {
            account_count: 19,
            writable_mask: 0b101_0101_0011_0000_0000,
            signer_mask: 0b000_0000_0000_0000_0001,
            outcome_mint_suffix: true,
            post_mint_accounts: 0,
            credit_creation_suffix: false,
        },
        FractionalRedemptionActionV1::RedeemInternalCredit => FractionalAccountContractV1 {
            account_count: 19,
            writable_mask: (1 << 8) | (1 << 9) | (1 << 12) | (1 << 13) | (1 << 14) | (1 << 15),
            signer_mask: 1,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: true,
        },
        FractionalRedemptionActionV1::RedeemBearerCredit => FractionalAccountContractV1 {
            account_count: 19,
            writable_mask: (1 << 8) | (1 << 9) | (1 << 12) | (1 << 14) | (1 << 16) | (1 << 18),
            signer_mask: 1,
            outcome_mint_suffix: true,
            post_mint_accounts: 4,
            credit_creation_suffix: true,
        },
        FractionalRedemptionActionV1::TransferCredit
        | FractionalRedemptionActionV1::MergeCredit => FractionalAccountContractV1 {
            account_count: 18,
            writable_mask: 0b00_1111_1111_1110_0011,
            signer_mask: 0b00_0000_0000_0000_0011,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: true,
        },
        FractionalRedemptionActionV1::CloseZeroCredit => FractionalAccountContractV1 {
            account_count: 10,
            writable_mask: 0b00_0111_1100,
            signer_mask: 0b00_0000_0001,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: false,
        },
        FractionalRedemptionActionV1::SealClaimsExhausted => FractionalAccountContractV1 {
            account_count: 12,
            writable_mask: 0b1001_0000_0000,
            signer_mask: 0,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: false,
        },
        FractionalRedemptionActionV1::CloseEmptyLedger => FractionalAccountContractV1 {
            account_count: 10,
            writable_mask: 0b11_1010_1011,
            signer_mask: 0,
            outcome_mint_suffix: false,
            post_mint_accounts: 0,
            credit_creation_suffix: false,
        },
    }
}

/// Minimal Solana account-meta projection for the capability boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolanaAccountMetaProjectionV1 {
    /// Exact account key.
    pub key: [u8; 32],
    /// Runtime writable bit.
    pub writable: bool,
    /// Runtime signer bit.
    pub signer: bool,
}

/// Fail closed before parsing payload bytes or inspecting any account meta.
///
/// A future activation must replace this function atomically with program
/// ownership/PDA/Resolution/ClaimLedger/Hoard/Position/Replay/token/rent adapters
/// and add the exact tuple to the release's capability manifest.
pub fn refuse_disabled_fractional_redemption_v1(
    _instruction_data: &[u8],
    _accounts: &[SolanaAccountMetaProjectionV1],
) -> Result<()> {
    Err(Error::CapabilityDisabled)
}

const _: () = assert!(FRACTIONAL_REDEMPTION_FAMILY_TAG == 79);
const _: () = assert!(FractionalRedemptionActionV1::FIRST_TAG == 1);
const _: () = assert!(FractionalRedemptionActionV1::LAST_TAG == 10);
