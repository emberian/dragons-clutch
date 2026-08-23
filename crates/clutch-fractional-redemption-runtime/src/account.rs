// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_collateral_adapter_v2::{BoundClaimIssuanceV1, BoundCollateralProfileV2};
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1, RentSplitV2};
use sha2::{Digest, Sha256};

use crate::codec::{
    exact, identity, put_identity, put_u128, put_u64, require_zeroes, u128_at, u64_at,
};
use crate::{map_retirement, Error, PayoutVectorV1, Result};

/// Fractional-redemption immutable policy discriminator.
pub const FRACTIONAL_POLICY_ACCOUNT_TAG: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_TAG;
/// Fractional-redemption immutable policy schema.
///
/// V1 is withdrawn: its offset 80 was allocated as a payout-vector digest.
/// V2 assigns that offset to the exact Resolution V5 data identity.
pub const FRACTIONAL_POLICY_ACCOUNT_VERSION: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_VERSION;
/// Exact immutable policy body width.
pub const FRACTIONAL_POLICY_ACCOUNT_BYTES: usize =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_BYTES;
/// Aggregate numerator-credit ledger discriminator.
pub const FRACTIONAL_LEDGER_ACCOUNT_TAG: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_TAG;
/// Aggregate numerator-credit ledger schema.
pub const FRACTIONAL_LEDGER_ACCOUNT_VERSION: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_VERSION;
/// Exact aggregate ledger body width.
pub const FRACTIONAL_LEDGER_ACCOUNT_BYTES: usize =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_BYTES;
/// Owner-scoped live numerator-credit discriminator.
pub const FRACTIONAL_CREDIT_ACCOUNT_TAG: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_TAG;
/// Owner-scoped live numerator-credit schema.
///
/// V1 is withdrawn: its offset 176 was allocated as a payout-vector digest.
/// V2 assigns that offset to the exact Resolution V5 data identity.
pub const FRACTIONAL_CREDIT_ACCOUNT_VERSION: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_VERSION;
/// Exact owner-scoped live credit body width.
pub const FRACTIONAL_CREDIT_ACCOUNT_BYTES: usize =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_BYTES;
/// Permanent zero-credit tombstone discriminator.
pub const FRACTIONAL_CREDIT_TOMBSTONE_TAG: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_TAG;
/// Permanent zero-credit tombstone schema.
///
/// V1 is withdrawn for the same offset-160 identity reinterpretation as the
/// live credit. Live and tombstone states therefore retain one V2 namespace.
pub const FRACTIONAL_CREDIT_TOMBSTONE_VERSION: u8 =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_VERSION;
/// Exact permanent credit tombstone width.
pub const FRACTIONAL_CREDIT_TOMBSTONE_BYTES: usize =
    clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_BYTES;

/// Canonical policy PDA seed prefix.
pub const FRACTIONAL_POLICY_PDA_PREFIX: &[u8] = b"fractional-redemption-policy:v2";
/// Canonical aggregate-ledger PDA seed prefix.
pub const FRACTIONAL_LEDGER_PDA_PREFIX: &[u8] = b"fractional-redemption-ledger:v1";
/// Canonical owner-credit/tombstone PDA seed prefix.
pub const FRACTIONAL_CREDIT_PDA_PREFIX: &[u8] = b"fractional-redemption-credit:v2";
/// Domain for the complete canonical `0xa5/v1` state identity.
pub const FRACTIONAL_LEDGER_STATE_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional-redemption/ledger-state/v1\0";
/// Domain for the complete canonical immutable `0xa4/v2` state identity.
pub const FRACTIONAL_POLICY_STATE_ID_DOMAIN_V2: &[u8] =
    b"dragons-clutch/fractional-redemption/policy-state/v2\0";

/// Honest no-subsidy terminal rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TerminalRemainderPolicyV1 {
    /// Keep claimant credits and claim backing live until same-domain voluntary
    /// aggregation turns every numerator into whole collateral atoms.
    RetainUntilExactAggregation = 1,
}

impl TerminalRemainderPolicyV1 {
    const fn wire_value(self) -> u8 {
        match self {
            Self::RetainUntilExactAggregation => 1,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::RetainUntilExactAggregation),
            _ => Err(Error::WrongPhase),
        }
    }
}

/// Aggregate-ledger lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FractionalLedgerPhaseV1 {
    /// Native claims remain and redemption is admitted.
    Live = 1,
    /// Canonical supply is zero; credits/backing remain until exact closure.
    ClaimsExhausted = 2,
}

impl FractionalLedgerPhaseV1 {
    const fn wire_value(self) -> u8 {
        match self {
            Self::Live => 1,
            Self::ClaimsExhausted => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Live),
            2 => Ok(Self::ClaimsExhausted),
            _ => Err(Error::WrongPhase),
        }
    }
}

/// V2 immutable policy binding Resolution to Realm-selected collateral and claims.
///
/// The vector itself is not stored here. `resolution_data_id` commits the exact
/// canonical Resolution V5 account and body; the body-only semantic ID and
/// vector are recomputed on every transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalPolicyV2 {
    /// Full successor MarketInstance identity.
    pub market_instance: Identity32V1,
    /// Canonical immutable Resolution account identity.
    pub resolution_account: Identity32V1,
    /// Account-bound data identity of the exact Resolution V5 body.
    pub resolution_data_id: Identity32V1,
    /// Immutable Realm identity.
    pub realm: Identity32V1,
    /// Realm-selected collateral-policy identity.
    pub collateral_policy: Identity32V1,
    /// Exact collateral adapter release identity.
    pub collateral_release: Identity32V1,
    /// Independent Token-2022 claim-issuance binding identity.
    pub claim_issuance_binding: Identity32V1,
    /// Nonzero Resolution/credit-accounting domain generation.
    pub domain_generation: u64,
    /// Exact minimal common lot for the bound resolved vector.
    pub common_lot: u64,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Explicit terminal remainder policy.
    pub terminal_policy: TerminalRemainderPolicyV1,
    /// Canonical policy PDA bump.
    pub stored_bump: u8,
    /// Payer-owned policy rent and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
}

impl FractionalPolicyV2 {
    /// Validate intrinsic account fields without duplicating the vector owner.
    pub fn validate(self) -> Result<()> {
        if self.domain_generation == 0
            || self.common_lot == 0
            || !(2..=16).contains(&self.outcome_count)
            || self.terminal_policy != TerminalRemainderPolicyV1::RetainUntilExactAggregation
        {
            return Err(Error::InvalidPayout);
        }
        self.rent.validate().map_err(map_retirement)
    }

    /// Join the immutable policy to the canonical payout and Realm-selected
    /// collateral plane.
    ///
    /// Internal Position redemption has no bearer-mint or Token-2022 effect,
    /// so it must not be made unavailable merely because a claim-program
    /// release is not present at runtime. The independent claim binding is
    /// still persisted (and required to be nonzero) and bearer routes perform
    /// the stronger [`Self::validate_join`] below.
    pub fn validate_internal_join(
        self,
        payout: PayoutVectorV1,
        resolution_data_id: Identity32V1,
        collateral: BoundCollateralProfileV2,
    ) -> Result<()> {
        self.validate()?;
        payout.validate()?;
        let market = collateral.market();
        let release_id = collateral
            .release()
            .id()
            .map_err(|_| Error::CollateralRefused)?;
        if self.market_instance.bytes() != market.market.bytes()
            || self.realm.bytes() != market.realm.bytes()
            || self.collateral_policy.bytes() != collateral.policy_id().bytes()
            || self.collateral_release.bytes() != release_id.bytes()
            || self.resolution_data_id != resolution_data_id
            || self.outcome_count != payout.outcome_count
            || self.common_lot != payout.common_lot()?
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Join the immutable policy to the canonical payout and both external
    /// adapters. Bearer routes require this stronger join because they mutate
    /// the independently released Token-2022 claim plane.
    pub fn validate_join(
        self,
        payout: PayoutVectorV1,
        resolution_data_id: Identity32V1,
        collateral: BoundCollateralProfileV2,
        claims: BoundClaimIssuanceV1,
    ) -> Result<()> {
        self.validate_internal_join(payout, resolution_data_id, collateral)?;
        if self.claim_issuance_binding.bytes() != claims.binding_id().bytes() {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exactly [`FRACTIONAL_POLICY_ACCOUNT_BYTES`] canonical bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_POLICY_ACCOUNT_BYTES]> {
        self.validate()?;
        let mut output = [0u8; FRACTIONAL_POLICY_ACCOUNT_BYTES];
        output[0] = FRACTIONAL_POLICY_ACCOUNT_TAG;
        output[1] = FRACTIONAL_POLICY_ACCOUNT_VERSION;
        output[2] = self.terminal_policy.wire_value();
        output[3] = self.outcome_count;
        output[4] = self.stored_bump;
        put_u64(&mut output, 8, self.domain_generation)?;
        for (offset, value) in [
            (16, self.market_instance),
            (48, self.resolution_account),
            (80, self.resolution_data_id),
            (112, self.realm),
            (144, self.collateral_policy),
            (176, self.collateral_release),
            (208, self.claim_issuance_binding),
        ] {
            put_identity(&mut output, offset, value)?;
        }
        put_u64(&mut output, 240, self.common_lot)?;
        output[248..].copy_from_slice(&self.rent.encode().map_err(map_retirement)?);
        Ok(output)
    }

    /// Decode hostile fixed bytes and refuse every noncanonical encoding.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_POLICY_ACCOUNT_BYTES)?;
        if input[0] != FRACTIONAL_POLICY_ACCOUNT_TAG {
            return Err(Error::WrongTag);
        }
        if input[1] != FRACTIONAL_POLICY_ACCOUNT_VERSION {
            return Err(Error::WrongVersion);
        }
        require_zeroes(input, 5, 8)?;
        let value = Self {
            market_instance: identity(input, 16)?,
            resolution_account: identity(input, 48)?,
            resolution_data_id: identity(input, 80)?,
            realm: identity(input, 112)?,
            collateral_policy: identity(input, 144)?,
            collateral_release: identity(input, 176)?,
            claim_issuance_binding: identity(input, 208)?,
            domain_generation: u64_at(input, 8)?,
            common_lot: u64_at(input, 240)?,
            outcome_count: input[3],
            terminal_policy: TerminalRemainderPolicyV1::decode(input[2])?,
            stored_bump: input[4],
            rent: DeletableRentOwnerV1::decode(&input[248..]).map_err(map_retirement)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hash the complete canonical immutable body for Product-root terminal
    /// authorization and anti-reinitialization.
    pub fn state_id(self) -> Result<Identity32V1> {
        let encoded = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(FRACTIONAL_POLICY_STATE_ID_DOMAIN_V2);
        hasher.update(encoded);
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)
    }

    /// Canonical PDA seed facts.
    pub const fn pda_seeds(self) -> FractionalPolicySeedsV2 {
        FractionalPolicySeedsV2 {
            market_instance: self.market_instance,
            resolution_account: self.resolution_account,
            resolution_data_id: self.resolution_data_id,
            stored_bump: self.stored_bump,
        }
    }
}

/// Sole persisted owner of aggregate numerator credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalLedgerV1 {
    /// Exact immutable policy PDA.
    pub policy_account: Identity32V1,
    /// Exact canonical ClaimLedger V3 account advanced with this ledger.
    pub claim_ledger_account: Identity32V1,
    /// Resolution/credit domain generation.
    pub domain_generation: u64,
    /// Exact next market-wide fractional action sequence.
    pub next_sequence: u64,
    /// Number of live owner-credit accounts.
    pub active_credit_accounts: u64,
    /// `K`, the exact sum of every live owner-credit numerator.
    pub aggregate_credit_numerator: u128,
    /// Live or claims-exhausted lifecycle.
    pub phase: FractionalLedgerPhaseV1,
    /// Canonical aggregate-ledger PDA bump.
    pub stored_bump: u8,
    /// Payer-owned ledger rent and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
}

impl FractionalLedgerV1 {
    /// Validate intrinsic account invariants.
    pub fn validate(self) -> Result<()> {
        if self.domain_generation == 0
            || self.next_sequence == 0
            || (self.active_credit_accounts == 0 && self.aggregate_credit_numerator != 0)
        {
            return Err(Error::AggregateMismatch);
        }
        self.rent.validate().map_err(map_retirement)
    }

    /// Bind the aggregate ledger to its immutable policy owner.
    pub fn validate_with_policy(
        self,
        policy_account: Identity32V1,
        policy: FractionalPolicyV2,
    ) -> Result<()> {
        self.validate()?;
        policy.validate()?;
        if self.policy_account != policy_account
            || self.domain_generation != policy.domain_generation
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Require and consume the exact next global sequence.
    pub fn advanced(self, expected_sequence: u64) -> Result<Self> {
        self.validate()?;
        if expected_sequence != self.next_sequence {
            return Err(Error::ReplayMismatch);
        }
        Ok(Self {
            next_sequence: self.next_sequence.checked_add(1).ok_or(Error::Arithmetic)?,
            ..self
        })
    }

    /// Encode exactly [`FRACTIONAL_LEDGER_ACCOUNT_BYTES`] canonical bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_LEDGER_ACCOUNT_BYTES]> {
        self.validate()?;
        let mut output = [0u8; FRACTIONAL_LEDGER_ACCOUNT_BYTES];
        output[0] = FRACTIONAL_LEDGER_ACCOUNT_TAG;
        output[1] = FRACTIONAL_LEDGER_ACCOUNT_VERSION;
        output[2] = self.phase.wire_value();
        output[3] = self.stored_bump;
        put_u64(&mut output, 8, self.domain_generation)?;
        put_u64(&mut output, 16, self.next_sequence)?;
        put_u64(&mut output, 24, self.active_credit_accounts)?;
        put_u128(&mut output, 32, self.aggregate_credit_numerator)?;
        for (offset, value) in [(48, self.policy_account), (80, self.claim_ledger_account)] {
            put_identity(&mut output, offset, value)?;
        }
        output[176..].copy_from_slice(&self.rent.encode().map_err(map_retirement)?);
        Ok(output)
    }

    /// Decode exact hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_LEDGER_ACCOUNT_BYTES)?;
        if input[0] != FRACTIONAL_LEDGER_ACCOUNT_TAG {
            return Err(Error::WrongTag);
        }
        if input[1] != FRACTIONAL_LEDGER_ACCOUNT_VERSION {
            return Err(Error::WrongVersion);
        }
        require_zeroes(input, 4, 8)?;
        require_zeroes(input, 112, 176)?;
        let value = Self {
            policy_account: identity(input, 48)?,
            claim_ledger_account: identity(input, 80)?,
            domain_generation: u64_at(input, 8)?,
            next_sequence: u64_at(input, 16)?,
            active_credit_accounts: u64_at(input, 24)?,
            aggregate_credit_numerator: u128_at(input, 32)?,
            phase: FractionalLedgerPhaseV1::decode(input[2])?,
            stored_bump: input[3],
            rent: DeletableRentOwnerV1::decode(&input[176..]).map_err(map_retirement)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hash the complete canonical account body for atomic ClaimLedger joins.
    pub fn state_id(self) -> Result<Identity32V1> {
        let encoded = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(FRACTIONAL_LEDGER_STATE_ID_DOMAIN_V1);
        hasher.update(encoded);
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)
    }

    /// Canonical PDA seed facts.
    pub const fn pda_seeds(self) -> FractionalLedgerSeedsV1 {
        FractionalLedgerSeedsV1 {
            policy_account: self.policy_account,
            stored_bump: self.stored_bump,
        }
    }
}

/// One live V2 owner-scoped numerator credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCreditV2 {
    /// Exact immutable policy PDA.
    pub policy_account: Identity32V1,
    /// Exact aggregate-credit ledger PDA.
    pub ledger_account: Identity32V1,
    /// Full successor MarketInstance identity.
    pub market_instance: Identity32V1,
    /// Canonical immutable Resolution account.
    pub resolution_account: Identity32V1,
    /// Exact PDA-bound Resolution V5 data identity.
    pub resolution_data_id: Identity32V1,
    /// Sole claimant owner of this numerator.
    pub claimant: Identity32V1,
    /// Resolution/credit accounting generation.
    pub domain_generation: u64,
    /// Monotone close/reopen generation of this claimant PDA.
    pub account_generation: u64,
    /// Exact next owner-credit action sequence.
    pub next_sequence: u64,
    /// Canonical residue numerator; joined validation requires `<D`.
    pub numerator: u64,
    /// Canonical owner-credit PDA bump.
    pub stored_bump: u8,
    /// Live-refund, permanent tombstone, and hostile-prefund compartments.
    pub rent: RentSplitV2,
}

impl FractionalCreditV2 {
    /// Validate intrinsic state and exact domain joins.
    pub fn validate_with(
        self,
        policy_account: Identity32V1,
        policy: FractionalPolicyV2,
        ledger_account: Identity32V1,
        ledger: FractionalLedgerV1,
        payout: PayoutVectorV1,
    ) -> Result<()> {
        self.rent.validate().map_err(|_| Error::RentRefused)?;
        if self.domain_generation == 0
            || self.account_generation == 0
            || self.next_sequence == 0
            || self.numerator >= payout.denominator
            || self.policy_account != policy_account
            || self.ledger_account != ledger_account
            || self.market_instance != policy.market_instance
            || self.resolution_account != policy.resolution_account
            || self.resolution_data_id != policy.resolution_data_id
            || self.domain_generation != policy.domain_generation
        {
            return Err(Error::MismatchedBinding);
        }
        ledger.validate_with_policy(policy_account, policy)?;
        Ok(())
    }

    /// Require and consume the exact next owner-credit sequence.
    pub fn advanced(self, expected_sequence: u64) -> Result<Self> {
        if expected_sequence != self.next_sequence {
            return Err(Error::ReplayMismatch);
        }
        Ok(Self {
            next_sequence: self.next_sequence.checked_add(1).ok_or(Error::Arithmetic)?,
            ..self
        })
    }

    /// Encode exactly [`FRACTIONAL_CREDIT_ACCOUNT_BYTES`] canonical bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_CREDIT_ACCOUNT_BYTES]> {
        if self.domain_generation == 0 || self.account_generation == 0 || self.next_sequence == 0 {
            return Err(Error::MismatchedBinding);
        }
        self.rent.validate().map_err(|_| Error::RentRefused)?;
        let mut output = [0u8; FRACTIONAL_CREDIT_ACCOUNT_BYTES];
        output[0] = FRACTIONAL_CREDIT_ACCOUNT_TAG;
        output[1] = FRACTIONAL_CREDIT_ACCOUNT_VERSION;
        output[2] = 1;
        output[3] = self.stored_bump;
        put_u64(&mut output, 8, self.domain_generation)?;
        put_u64(&mut output, 16, self.account_generation)?;
        put_u64(&mut output, 24, self.next_sequence)?;
        put_u64(&mut output, 32, self.numerator)?;
        for (offset, value) in [
            (48, self.policy_account),
            (80, self.ledger_account),
            (112, self.market_instance),
            (144, self.resolution_account),
            (176, self.resolution_data_id),
            (208, self.claimant),
        ] {
            put_identity(&mut output, offset, value)?;
        }
        output[240..].copy_from_slice(&self.rent.encode().map_err(|_| Error::RentRefused)?);
        Ok(output)
    }

    /// Decode exact hostile bytes. The denominator join remains mandatory
    /// before the decoded value is authoritative.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_CREDIT_ACCOUNT_BYTES)?;
        if input[0] != FRACTIONAL_CREDIT_ACCOUNT_TAG {
            return Err(Error::WrongTag);
        }
        if input[1] != FRACTIONAL_CREDIT_ACCOUNT_VERSION {
            return Err(Error::WrongVersion);
        }
        if input[2] != 1 {
            return Err(Error::WrongPhase);
        }
        require_zeroes(input, 4, 8)?;
        require_zeroes(input, 40, 48)?;
        let value = Self {
            policy_account: identity(input, 48)?,
            ledger_account: identity(input, 80)?,
            market_instance: identity(input, 112)?,
            resolution_account: identity(input, 144)?,
            resolution_data_id: identity(input, 176)?,
            claimant: identity(input, 208)?,
            domain_generation: u64_at(input, 8)?,
            account_generation: u64_at(input, 16)?,
            next_sequence: u64_at(input, 24)?,
            numerator: u64_at(input, 32)?,
            stored_bump: input[3],
            rent: RentSplitV2::decode(&input[240..]).map_err(|_| Error::RentRefused)?,
        };
        if value.domain_generation == 0 || value.account_generation == 0 || value.next_sequence == 0
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(value)
    }

    /// Canonical PDA seed facts shared with the permanent tombstone.
    pub const fn pda_seeds(self) -> FractionalCreditSeedsV2 {
        FractionalCreditSeedsV2 {
            policy_account: self.policy_account,
            claimant: self.claimant,
            stored_bump: self.stored_bump,
        }
    }
}

/// V2 permanent replay-prevention identity after a zero-only credit close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCreditTombstoneV2 {
    /// Exact immutable policy PDA.
    pub policy_account: Identity32V1,
    /// Exact aggregate-credit ledger PDA.
    pub ledger_account: Identity32V1,
    /// Full successor MarketInstance identity.
    pub market_instance: Identity32V1,
    /// Canonical immutable Resolution account.
    pub resolution_account: Identity32V1,
    /// Exact PDA-bound Resolution V5 data identity.
    pub resolution_data_id: Identity32V1,
    /// Claimant whose namespace cannot be resurrected by stale instructions.
    pub claimant: Identity32V1,
    /// Resolution/credit accounting generation.
    pub domain_generation: u64,
    /// Closed owner-credit generation.
    pub account_generation: u64,
    /// First sequence not consumable by the retired generation.
    pub closed_next_sequence: u64,
    /// Canonical owner-credit PDA bump.
    pub stored_bump: u8,
    /// Permanent lamport principal retained in this tombstone.
    pub permanent_tombstone_principal: u64,
}

impl FractionalCreditTombstoneV2 {
    /// Validate permanent identity and funding.
    pub fn validate(self) -> Result<()> {
        if self.domain_generation == 0
            || self.account_generation == 0
            || self.closed_next_sequence == 0
            || self.permanent_tombstone_principal == 0
        {
            return Err(Error::TombstoneRequired);
        }
        Ok(())
    }

    /// Encode exactly [`FRACTIONAL_CREDIT_TOMBSTONE_BYTES`] canonical bytes.
    pub fn encode(self) -> Result<[u8; FRACTIONAL_CREDIT_TOMBSTONE_BYTES]> {
        self.validate()?;
        let mut output = [0u8; FRACTIONAL_CREDIT_TOMBSTONE_BYTES];
        output[0] = FRACTIONAL_CREDIT_TOMBSTONE_TAG;
        output[1] = FRACTIONAL_CREDIT_TOMBSTONE_VERSION;
        output[2] = 1;
        output[3] = self.stored_bump;
        put_u64(&mut output, 8, self.domain_generation)?;
        put_u64(&mut output, 16, self.account_generation)?;
        put_u64(&mut output, 24, self.closed_next_sequence)?;
        for (offset, value) in [
            (32, self.policy_account),
            (64, self.ledger_account),
            (96, self.market_instance),
            (128, self.resolution_account),
            (160, self.resolution_data_id),
            (192, self.claimant),
        ] {
            put_identity(&mut output, offset, value)?;
        }
        put_u64(&mut output, 224, self.permanent_tombstone_principal)?;
        Ok(output)
    }

    /// Decode exact hostile tombstone bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(input, FRACTIONAL_CREDIT_TOMBSTONE_BYTES)?;
        if input[0] != FRACTIONAL_CREDIT_TOMBSTONE_TAG {
            return Err(Error::WrongTag);
        }
        if input[1] != FRACTIONAL_CREDIT_TOMBSTONE_VERSION {
            return Err(Error::WrongVersion);
        }
        if input[2] != 1 {
            return Err(Error::WrongPhase);
        }
        require_zeroes(input, 4, 8)?;
        let value = Self {
            policy_account: identity(input, 32)?,
            ledger_account: identity(input, 64)?,
            market_instance: identity(input, 96)?,
            resolution_account: identity(input, 128)?,
            resolution_data_id: identity(input, 160)?,
            claimant: identity(input, 192)?,
            domain_generation: u64_at(input, 8)?,
            account_generation: u64_at(input, 16)?,
            closed_next_sequence: u64_at(input, 24)?,
            stored_bump: input[3],
            permanent_tombstone_principal: u64_at(input, 224)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Canonical PDA seed facts shared with the live credit.
    pub const fn pda_seeds(self) -> FractionalCreditSeedsV2 {
        FractionalCreditSeedsV2 {
            policy_account: self.policy_account,
            claimant: self.claimant,
            stored_bump: self.stored_bump,
        }
    }
}

/// Canonical V2 immutable policy PDA seed tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalPolicySeedsV2 {
    market_instance: Identity32V1,
    resolution_account: Identity32V1,
    resolution_data_id: Identity32V1,
    stored_bump: u8,
}

impl FractionalPolicySeedsV2 {
    /// Static seed prefix.
    pub const fn prefix(self) -> &'static [u8] {
        FRACTIONAL_POLICY_PDA_PREFIX
    }
    /// Full successor MarketInstance seed.
    pub const fn market_instance(self) -> Identity32V1 {
        self.market_instance
    }
    /// Canonical Resolution-account seed.
    pub const fn resolution_account(self) -> Identity32V1 {
        self.resolution_account
    }
    /// Exact PDA-bound Resolution V5 data seed.
    pub const fn resolution_data_id(self) -> Identity32V1 {
        self.resolution_data_id
    }
    /// Stored bump.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

/// Canonical aggregate-ledger PDA seed tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalLedgerSeedsV1 {
    policy_account: Identity32V1,
    stored_bump: u8,
}

impl FractionalLedgerSeedsV1 {
    /// Static seed prefix.
    pub const fn prefix(self) -> &'static [u8] {
        FRACTIONAL_LEDGER_PDA_PREFIX
    }
    /// Immutable policy PDA seed.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }
    /// Stored bump.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

/// Canonical V2 owner-credit/tombstone PDA seed tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCreditSeedsV2 {
    policy_account: Identity32V1,
    claimant: Identity32V1,
    stored_bump: u8,
}

impl FractionalCreditSeedsV2 {
    /// Static seed prefix.
    pub const fn prefix(self) -> &'static [u8] {
        FRACTIONAL_CREDIT_PDA_PREFIX
    }
    /// Immutable policy PDA seed.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }
    /// Exact claimant seed.
    pub const fn claimant(self) -> Identity32V1 {
        self.claimant
    }
    /// Stored bump.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

const _: () = assert!(FRACTIONAL_POLICY_ACCOUNT_BYTES == 248 + 48);
const _: () = assert!(FRACTIONAL_LEDGER_ACCOUNT_BYTES == 176 + 48);
const _: () = assert!(FRACTIONAL_CREDIT_ACCOUNT_BYTES == 240 + 56);
const _: () = assert!(FRACTIONAL_CREDIT_TOMBSTONE_BYTES == 224 + 8);
