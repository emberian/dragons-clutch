// SPDX-License-Identifier: AGPL-3.0-or-later

//! Full-width Market collateral and native-claim liability successors.
//!
//! `HoardV2` owns classified collateral liabilities, never token-program
//! balance bytes: the runtime must reload the selected token account and prove
//! that its visible amount covers `cash + locked`. `ClaimLedgerV3` owns exact
//! aggregate native-Egg supply. Fractional numerator credit and its live count
//! remain solely in the separately bound fractional ledger; neither fact is
//! copied here.

use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionV3Sha256Backend, DELETABLE_RENT_OWNER_V1_BYTES,
    MAX_OUTCOMES,
};

use crate::codec::{Reader, Writer};
use crate::{
    digest, AcceptedClaimRedemptionCollateralV2, AcceptedPositionCollateralTransferV3,
    CustodyTransferKindV2, Error, Id, Result,
};

/// Reused historical Hoard discriminator under the full-width V2 layout.
pub const HOARD_V2_TAG: u8 = 0x05;
/// Full-width Hoard layout version.
pub const HOARD_V2_VERSION: u8 = 2;
/// Exact canonical Hoard V2 bytes.
pub const HOARD_V2_BYTES: usize = 312;
/// Reused reference-kernel discriminator under the ClaimLedger V3 layout.
pub const CLAIM_LEDGER_V3_TAG: u8 = 0x41;
/// Full-width ClaimLedger layout version.
pub const CLAIM_LEDGER_V3_VERSION: u8 = 3;
/// Exact canonical ClaimLedger V3 bytes.
pub const CLAIM_LEDGER_V3_BYTES: usize = 552;

/// Hoard V2 semantic identity domain.
pub const HOARD_V2_SEMANTIC_DOMAIN: &[u8] = b"dragons-clutch/hoard/v2\0";
/// ClaimLedger V3 semantic identity domain.
pub const CLAIM_LEDGER_V3_SEMANTIC_DOMAIN: &[u8] = b"dragons-clutch/claim-ledger/v3\0";
/// Exact classified-cash transition receipt domain.
pub const HOARD_CASH_LIABILITY_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/hoard/cash-liability/v2\0";
/// Accepted Position/Hoard/custody cash-transition domain.
pub const POSITION_HOARD_CASH_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/position-hoard/cash-transition/v3\0";
/// Exact complete-set reclassification receipt domain.
pub const COMPLETE_SET_RECLASSIFICATION_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-ledger/complete-set-reclassification/v3\0";
/// Fractional-ledger/ClaimLedger atomic successor domain.
pub const FRACTIONAL_CLAIM_LEDGER_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-ledger/fractional-transition/v3\0";
/// Fractional claim payout and locked-principal release domain.
pub const FRACTIONAL_CLAIM_REDEMPTION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-ledger/fractional-redemption/v3\0";
/// Explicit absent-0xa5 founding join domain.
pub const FRACTIONAL_CLAIM_LEDGER_FOUNDING_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-ledger/fractional-founding/v3\0";
/// Exhausted ClaimLedger/0xa5 retirement transition domain.
pub const FRACTIONAL_CLAIM_LEDGER_RETIREMENT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/claim-ledger/fractional-retirement/v3\0";
/// Exact absent-account Market-liability founding plan domain.
pub const MARKET_LIABILITY_FOUNDING_DOMAIN_V3: &[u8] =
    b"dragons-clutch/market-liability/founding/v3\0";

const HEADER_BYTES: usize = 16;
const HOARD_ID_COUNT: usize = 7;
const CLAIM_LEDGER_ID_COUNT: usize = 6;
const _: () = assert!(
    HOARD_V2_BYTES == HEADER_BYTES + HOARD_ID_COUNT * 32 + 3 * 8 + DELETABLE_RENT_OWNER_V1_BYTES
);
const _: () = assert!(
    CLAIM_LEDGER_V3_BYTES
        == HEADER_BYTES
            + CLAIM_LEDGER_ID_COUNT * 32
            + 2 * MAX_OUTCOMES * 8
            + 8
            + 32
            + DELETABLE_RENT_OWNER_V1_BYTES
);

/// Shared full-width Market liability lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketLiabilityLifecycleV1 {
    /// Trading and new complete-set creation are admitted.
    Open = 1,
    /// Resolution is frozen; exits and liability destruction remain admitted.
    Resolved = 2,
    /// New economic mutation is disabled while terminal absence is proved.
    Retiring = 3,
}

impl MarketLiabilityLifecycleV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Open),
            2 => Ok(Self::Resolved),
            3 => Ok(Self::Retiring),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Canonical full-width Market Hoard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoardV2 {
    /// Full MarketInstanceV2 identity.
    pub market_instance_id: Id,
    /// Immutable Realm identity selecting collateral semantics.
    pub realm_id: Id,
    /// Immutable ProfileV2 identity.
    pub profile_id: Id,
    /// Exact collateral-policy content identity.
    pub collateral_policy_id: Id,
    /// Exact compiled collateral-release identity.
    pub collateral_release_id: Id,
    /// Sole program-derived custody signer.
    pub authority: Id,
    /// Exact external collateral token account.
    pub token_account: Id,
    /// Market cap applied only to locked claim principal.
    pub collateral_cap_atoms: u64,
    /// Aggregate Position cash liability classified inside custody.
    pub cash_liability_atoms: u64,
    /// Collateral principal locked against outstanding native claims.
    pub locked_claim_principal_atoms: u64,
    /// Shared liability lifecycle.
    pub lifecycle: MarketLiabilityLifecycleV1,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Canonical Hoard PDA bump.
    pub stored_bump: u8,
    /// Deletable lamport-rent owner; never collateral principal.
    pub rent: DeletableRentOwnerV1,
}

impl HoardV2 {
    /// Validate identities, cap, coverage arithmetic, and rent ownership.
    pub fn validate(self) -> Result<()> {
        for id in [
            self.market_instance_id,
            self.realm_id,
            self.profile_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.authority,
            self.token_account,
        ] {
            id.require_live()?;
        }
        if usize::from(self.outcome_count) == 0
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.locked_claim_principal_atoms > self.collateral_cap_atoms
        {
            return Err(Error::InvalidParameter);
        }
        self.cash_liability_atoms
            .checked_add(self.locked_claim_principal_atoms)
            .ok_or(Error::Arithmetic)?;
        self.rent.validate().map_err(|_| Error::InvalidParameter)
    }

    /// Required visible token balance; unsolicited surplus is not a liability.
    pub fn required_custody_atoms(self) -> Result<u64> {
        self.validate()?;
        self.cash_liability_atoms
            .checked_add(self.locked_claim_principal_atoms)
            .ok_or(Error::Arithmetic)
    }

    /// Encode exactly [`HOARD_V2_BYTES`] canonical bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, HOARD_V2_BYTES)?;
        write_header(
            &mut writer,
            HOARD_V2_TAG,
            HOARD_V2_VERSION,
            self.lifecycle,
            self.outcome_count,
            self.stored_bump,
        )?;
        for id in [
            self.market_instance_id,
            self.realm_id,
            self.profile_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.authority,
            self.token_account,
        ] {
            writer.id(id)?;
        }
        writer.u64(self.collateral_cap_atoms)?;
        writer.u64(self.cash_liability_atoms)?;
        writer.u64(self.locked_claim_principal_atoms)?;
        writer.bytes(&self.rent.encode().map_err(|_| Error::InvalidParameter)?)?;
        writer.finish()
    }

    /// Decode exactly [`HOARD_V2_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, HOARD_V2_BYTES)?;
        let (lifecycle, outcome_count, stored_bump) =
            read_header(&mut reader, HOARD_V2_TAG, HOARD_V2_VERSION)?;
        let value = Self {
            market_instance_id: reader.id()?,
            realm_id: reader.id()?,
            profile_id: reader.id()?,
            collateral_policy_id: reader.id()?,
            collateral_release_id: reader.id()?,
            authority: reader.id()?,
            token_account: reader.id()?,
            collateral_cap_atoms: reader.u64()?,
            cash_liability_atoms: reader.u64()?,
            locked_claim_principal_atoms: reader.u64()?,
            lifecycle,
            outcome_count,
            stored_bump,
            rent: DeletableRentOwnerV1::decode(&reader.bytes::<DELETABLE_RENT_OWNER_V1_BYTES>()?)
                .map_err(|_| Error::InvalidParameter)?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Canonical semantic ID of these exact bytes.
    pub fn semantic_id<B: PositionV3Sha256Backend>(self, backend: &B) -> Result<Id> {
        let mut body = [0; HOARD_V2_BYTES];
        self.encode(&mut body)?;
        let id = Id::from_bytes(backend.sha256(HOARD_V2_SEMANTIC_DOMAIN, &body));
        id.require_live()?;
        Ok(id)
    }
}

/// Full-width aggregate native-claim liability owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimLedgerV3 {
    /// Full MarketInstanceV2 identity.
    pub market_instance_id: Id,
    /// Immutable Realm identity.
    pub realm_id: Id,
    /// Exact NativeClaimBasis body identity.
    pub native_claim_basis_id: Id,
    /// Immutable fractional-credit policy identity.
    pub fractional_policy_id: Id,
    /// Exact 0xa5 fractional ledger account; it alone owns K and live count.
    pub fractional_ledger_account: Id,
    /// Exact Resolution account, zero only before resolution.
    pub resolution_account: Id,
    /// Aggregate Position-owned native claims by outcome.
    pub aggregate_internal_supply: [u64; MAX_OUTCOMES],
    /// Aggregate materialized bearer claims by outcome.
    pub aggregate_materialized_supply: [u64; MAX_OUTCOMES],
    /// Exact next ordinal consumed by an atomic 0xa5 fractional mutation.
    pub next_fractional_sequence: u64,
    /// Last atomic 0xa5/ClaimLedger transition; zero only at founding.
    pub last_fractional_transition_id: Id,
    /// Shared liability lifecycle.
    pub lifecycle: MarketLiabilityLifecycleV1,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Canonical ClaimLedger PDA bump.
    pub stored_bump: u8,
    /// Deletable lamport-rent owner; never collateral principal.
    pub rent: DeletableRentOwnerV1,
}

impl ClaimLedgerV3 {
    /// Validate identities, lifecycle resolution presence, vectors, and rent.
    pub fn validate(self) -> Result<()> {
        for id in [
            self.market_instance_id,
            self.realm_id,
            self.native_claim_basis_id,
            self.fractional_policy_id,
            self.fractional_ledger_account,
        ] {
            id.require_live()?;
        }
        if usize::from(self.outcome_count) == 0
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || (self.lifecycle == MarketLiabilityLifecycleV1::Open
                && !self.resolution_account.is_zero())
            || (self.lifecycle != MarketLiabilityLifecycleV1::Open
                && self.resolution_account.is_zero())
        {
            return Err(Error::InvalidParameter);
        }
        let mut index = usize::from(self.outcome_count);
        while index < MAX_OUTCOMES {
            if self.aggregate_internal_supply[index] != 0
                || self.aggregate_materialized_supply[index] != 0
            {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if (self.next_fractional_sequence == 0 && !self.last_fractional_transition_id.is_zero())
            || (self.next_fractional_sequence != 0 && self.last_fractional_transition_id.is_zero())
        {
            return Err(Error::InvalidParameter);
        }
        self.rent.validate().map_err(|_| Error::InvalidParameter)
    }

    /// Encode exactly [`CLAIM_LEDGER_V3_BYTES`] canonical bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, CLAIM_LEDGER_V3_BYTES)?;
        write_header(
            &mut writer,
            CLAIM_LEDGER_V3_TAG,
            CLAIM_LEDGER_V3_VERSION,
            self.lifecycle,
            self.outcome_count,
            self.stored_bump,
        )?;
        for id in [
            self.market_instance_id,
            self.realm_id,
            self.native_claim_basis_id,
            self.fractional_policy_id,
            self.fractional_ledger_account,
            self.resolution_account,
        ] {
            writer.id(id)?;
        }
        for amount in self.aggregate_internal_supply {
            writer.u64(amount)?;
        }
        for amount in self.aggregate_materialized_supply {
            writer.u64(amount)?;
        }
        writer.u64(self.next_fractional_sequence)?;
        writer.id(self.last_fractional_transition_id)?;
        writer.bytes(&self.rent.encode().map_err(|_| Error::InvalidParameter)?)?;
        writer.finish()
    }

    /// Decode exactly [`CLAIM_LEDGER_V3_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, CLAIM_LEDGER_V3_BYTES)?;
        let (lifecycle, outcome_count, stored_bump) =
            read_header(&mut reader, CLAIM_LEDGER_V3_TAG, CLAIM_LEDGER_V3_VERSION)?;
        let market_instance_id = reader.id()?;
        let realm_id = reader.id()?;
        let native_claim_basis_id = reader.id()?;
        let fractional_policy_id = reader.id()?;
        let fractional_ledger_account = reader.id()?;
        let resolution_account = reader.id()?;
        let mut aggregate_internal_supply = [0; MAX_OUTCOMES];
        let mut aggregate_materialized_supply = [0; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            aggregate_internal_supply[index] = reader.u64()?;
            index += 1;
        }
        index = 0;
        while index < MAX_OUTCOMES {
            aggregate_materialized_supply[index] = reader.u64()?;
            index += 1;
        }
        let next_fractional_sequence = reader.u64()?;
        let last_fractional_transition_id = reader.id()?;
        let value = Self {
            market_instance_id,
            realm_id,
            native_claim_basis_id,
            fractional_policy_id,
            fractional_ledger_account,
            resolution_account,
            aggregate_internal_supply,
            aggregate_materialized_supply,
            next_fractional_sequence,
            last_fractional_transition_id,
            lifecycle,
            outcome_count,
            stored_bump,
            rent: DeletableRentOwnerV1::decode(&reader.bytes::<DELETABLE_RENT_OWNER_V1_BYTES>()?)
                .map_err(|_| Error::InvalidParameter)?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Canonical semantic ID of these exact bytes.
    pub fn semantic_id<B: PositionV3Sha256Backend>(self, backend: &B) -> Result<Id> {
        let mut body = [0; CLAIM_LEDGER_V3_BYTES];
        self.encode(&mut body)?;
        let id = Id::from_bytes(backend.sha256(CLAIM_LEDGER_V3_SEMANTIC_DOMAIN, &body));
        id.require_live()?;
        Ok(id)
    }

    /// Total outstanding native claims for one active outcome.
    pub fn outcome_supply(self, outcome: u8) -> Result<u64> {
        self.validate()?;
        if outcome >= self.outcome_count {
            return Err(Error::InvalidParameter);
        }
        self.aggregate_internal_supply[usize::from(outcome)]
            .checked_add(self.aggregate_materialized_supply[usize::from(outcome)])
            .ok_or(Error::Arithmetic)
    }
}

/// Exact non-persisted identities and rent owners required to found the two
/// canonical Market-liability accounts.
///
/// This value is not authority. The constructor also requires a private-field
/// [`crate::BoundCollateralProfileV2`] produced by the runtime's authenticated
/// Realm/Profile/policy/release join. The SBF adapter must separately prove
/// that both named accounts are absent canonical PDAs before allocating them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLiabilityFoundingRequestV3 {
    /// Exact absent HoardV2 account.
    pub hoard_account: Id,
    /// Exact absent ClaimLedgerV3 account.
    pub claim_ledger_account: Id,
    /// Full Product-owned MarketInstanceV2 identity.
    pub market_instance_id: Id,
    /// Exact Product-owned NativeClaimBasis identity.
    pub native_claim_basis_id: Id,
    /// Immutable fractional-credit policy identity.
    pub fractional_policy_id: Id,
    /// Exact canonical fractional-ledger account, initially absent.
    pub fractional_ledger_account: Id,
    /// Exact General MarketRuntime authority for outcome claim mints.
    pub claim_mint_authority: Id,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Canonical HoardV2 PDA bump.
    pub hoard_bump: u8,
    /// Canonical ClaimLedgerV3 PDA bump.
    pub claim_ledger_bump: u8,
    /// Separately admitted HoardV2 lamport-rent owner.
    pub hoard_rent: DeletableRentOwnerV1,
    /// Separately admitted ClaimLedgerV3 lamport-rent owner.
    pub claim_ledger_rent: DeletableRentOwnerV1,
}

/// Closed founding poststate for the full-width Market liability plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLiabilityFoundingPlanV3 {
    hoard_account: Id,
    claim_ledger_account: Id,
    claim_mint_authority: Id,
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    hoard_id: Id,
    claim_ledger_id: Id,
    founding_id: Id,
}

impl MarketLiabilityFoundingPlanV3 {
    /// Exact absent HoardV2 account the runtime must create.
    pub const fn hoard_account(self) -> Id {
        self.hoard_account
    }

    /// Exact absent ClaimLedgerV3 account the runtime must create.
    pub const fn claim_ledger_account(self) -> Id {
        self.claim_ledger_account
    }

    /// General MarketRuntime authority selected for all outcome claim mints.
    pub const fn claim_mint_authority(self) -> Id {
        self.claim_mint_authority
    }

    /// Complete canonical zero-liability Hoard poststate.
    pub const fn hoard(self) -> HoardV2 {
        self.hoard
    }

    /// Complete canonical zero-supply ClaimLedger poststate.
    pub const fn claim_ledger(self) -> ClaimLedgerV3 {
        self.claim_ledger
    }

    /// Semantic identity of the exact Hoard poststate.
    pub const fn hoard_id(self) -> Id {
        self.hoard_id
    }

    /// Semantic identity of the exact ClaimLedger poststate.
    pub const fn claim_ledger_id(self) -> Id {
        self.claim_ledger_id
    }

    /// Shared identity joining both absent accounts, rent owners, and the
    /// separately selected claim-mint authority.
    pub const fn founding_id(self) -> Id {
        self.founding_id
    }
}

/// Prepare the only canonical founding poststate for HoardV2 and
/// ClaimLedgerV3.
///
/// Cash, locked principal, native supply, the fractional latch, and the
/// Resolution link all begin at zero. In particular, this function cannot
/// endow a founding Position, mint a claim, or source rent from collateral.
pub fn prepare_market_liability_founding_v3<B: PositionV3Sha256Backend>(
    bound: crate::BoundCollateralProfileV2,
    request: MarketLiabilityFoundingRequestV3,
    backend: &B,
) -> Result<MarketLiabilityFoundingPlanV3> {
    for id in [
        request.hoard_account,
        request.claim_ledger_account,
        request.market_instance_id,
        request.native_claim_basis_id,
        request.fractional_policy_id,
        request.fractional_ledger_account,
        request.claim_mint_authority,
    ] {
        id.require_live()?;
    }
    request
        .hoard_rent
        .validate()
        .map_err(|_| Error::InvalidParameter)?;
    request
        .claim_ledger_rent
        .validate()
        .map_err(|_| Error::InvalidParameter)?;

    let market = bound.market();
    let realm = bound.realm_bound().realm();
    if market.market != request.market_instance_id
        || market.realm != realm.realm
        || market.profile != realm.profile
        || request.outcome_count == 0
        || usize::from(request.outcome_count) > MAX_OUTCOMES
    {
        return Err(Error::MismatchedBinding);
    }

    let hoard = HoardV2 {
        market_instance_id: market.market,
        realm_id: market.realm,
        profile_id: market.profile,
        collateral_policy_id: bound.policy_id(),
        collateral_release_id: bound.release().id()?,
        authority: market.hoard_authority,
        token_account: market.hoard_token_account,
        collateral_cap_atoms: market.collateral_cap_atoms,
        cash_liability_atoms: 0,
        locked_claim_principal_atoms: 0,
        lifecycle: MarketLiabilityLifecycleV1::Open,
        outcome_count: request.outcome_count,
        stored_bump: request.hoard_bump,
        rent: request.hoard_rent,
    };
    let claim_ledger = ClaimLedgerV3 {
        market_instance_id: market.market,
        realm_id: market.realm,
        native_claim_basis_id: request.native_claim_basis_id,
        fractional_policy_id: request.fractional_policy_id,
        fractional_ledger_account: request.fractional_ledger_account,
        resolution_account: Id::ZERO,
        aggregate_internal_supply: [0; MAX_OUTCOMES],
        aggregate_materialized_supply: [0; MAX_OUTCOMES],
        next_fractional_sequence: 0,
        last_fractional_transition_id: Id::ZERO,
        lifecycle: MarketLiabilityLifecycleV1::Open,
        outcome_count: request.outcome_count,
        stored_bump: request.claim_ledger_bump,
        rent: request.claim_ledger_rent,
    };
    hoard.validate()?;
    claim_ledger.validate()?;
    let hoard_id = hoard.semantic_id(backend)?;
    let claim_ledger_id = claim_ledger.semantic_id(backend)?;
    let founding_id = digest(
        MARKET_LIABILITY_FOUNDING_DOMAIN_V3,
        &[
            &request.hoard_account.bytes(),
            &request.claim_ledger_account.bytes(),
            &request.claim_mint_authority.bytes(),
            &hoard_id.bytes(),
            &claim_ledger_id.bytes(),
        ],
    );
    founding_id.require_live()?;
    Ok(MarketLiabilityFoundingPlanV3 {
        hoard_account: request.hoard_account,
        claim_ledger_account: request.claim_ledger_account,
        claim_mint_authority: request.claim_mint_authority,
        hoard,
        claim_ledger,
        hoard_id,
        claim_ledger_id,
        founding_id,
    })
}

/// Exact native-supply effect paired with one 0xa5 fractional mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalClaimSupplyMutationV3 {
    /// Credit transfer/claim/close changes only K/count in 0xa5.
    Unchanged,
    /// Burn Position-owned native claims at one outcome.
    BurnInternal { outcome: u8, amount: u64 },
    /// Burn materialized bearer claims at one outcome.
    BurnMaterialized { outcome: u8, amount: u64 },
}

/// Exact atomic ClaimLedger half of one separately authenticated 0xa5 plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimLedgerPlanV3 {
    /// ClaimLedger semantic ID before the transition.
    claim_ledger_before_id: Id,
    /// Exact 0xa5 semantic ID before the transition.
    fractional_ledger_before_id: Id,
    /// Complete permitted ClaimLedger successor.
    claim_ledger_after: ClaimLedgerV3,
    /// ClaimLedger semantic ID after the transition.
    claim_ledger_after_id: Id,
    /// Exact 0xa5 semantic ID after the transition.
    fractional_ledger_after_id: Id,
    /// Shared transition identity persisted as the ClaimLedger latch.
    transition_id: Id,
    /// Exact consumed cross-ledger ordinal.
    consumed_sequence: u64,
}

impl FractionalClaimLedgerPlanV3 {
    /// ClaimLedger semantic ID before the transition.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.claim_ledger_before_id
    }

    /// Exact 0xa5 semantic ID before the transition.
    pub const fn fractional_ledger_before_id(self) -> Id {
        self.fractional_ledger_before_id
    }

    /// Complete permitted ClaimLedger successor.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.claim_ledger_after
    }

    /// ClaimLedger semantic ID after the transition.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.claim_ledger_after_id
    }

    /// Exact 0xa5 semantic ID after the transition.
    pub const fn fractional_ledger_after_id(self) -> Id {
        self.fractional_ledger_after_id
    }

    /// Shared transition identity persisted as the ClaimLedger latch.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }

    /// Exact consumed cross-ledger ordinal.
    pub const fn consumed_sequence(self) -> u64 {
        self.consumed_sequence
    }
}

/// Project the ClaimLedger successor paired with exact 0xa5 pre/post IDs.
///
/// Runtime authority is intentionally external: the Fractional owner must
/// rederive its private K/count successor and the SBF wrapper must commit both
/// accounts atomically. This function makes the ClaimLedger half total and
/// prevents either side from advancing under a different ordinal or receipt.
pub fn prepare_fractional_claim_ledger_successor_v3<B: PositionV3Sha256Backend>(
    claim_ledger: ClaimLedgerV3,
    fractional_ledger_before_id: Id,
    fractional_ledger_after_id: Id,
    consumed_sequence: u64,
    mutation: FractionalClaimSupplyMutationV3,
    backend: &B,
) -> Result<FractionalClaimLedgerPlanV3> {
    claim_ledger.validate()?;
    fractional_ledger_before_id.require_live()?;
    fractional_ledger_after_id.require_live()?;
    if fractional_ledger_before_id == fractional_ledger_after_id
        || consumed_sequence != claim_ledger.next_fractional_sequence
        || claim_ledger.lifecycle == MarketLiabilityLifecycleV1::Retiring
    {
        return Err(Error::MismatchedBinding);
    }
    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let mut aggregate_internal_supply = claim_ledger.aggregate_internal_supply;
    let mut aggregate_materialized_supply = claim_ledger.aggregate_materialized_supply;
    let (kind, outcome, amount) = match mutation {
        FractionalClaimSupplyMutationV3::Unchanged => (0u8, 0u8, 0u64),
        FractionalClaimSupplyMutationV3::BurnInternal { outcome, amount } => {
            if outcome >= claim_ledger.outcome_count || amount == 0 {
                return Err(Error::InvalidParameter);
            }
            aggregate_internal_supply[usize::from(outcome)] = aggregate_internal_supply
                [usize::from(outcome)]
            .checked_sub(amount)
            .ok_or(Error::AggregateLiabilityInsufficient)?;
            (1, outcome, amount)
        }
        FractionalClaimSupplyMutationV3::BurnMaterialized { outcome, amount } => {
            if outcome >= claim_ledger.outcome_count || amount == 0 {
                return Err(Error::InvalidParameter);
            }
            aggregate_materialized_supply[usize::from(outcome)] = aggregate_materialized_supply
                [usize::from(outcome)]
            .checked_sub(amount)
            .ok_or(Error::AggregateLiabilityInsufficient)?;
            (2, outcome, amount)
        }
    };
    let next_fractional_sequence = consumed_sequence.checked_add(1).ok_or(Error::Arithmetic)?;
    let transition_id = digest(
        FRACTIONAL_CLAIM_LEDGER_TRANSITION_DOMAIN_V3,
        &[
            &claim_ledger.market_instance_id.bytes(),
            &claim_ledger.fractional_policy_id.bytes(),
            &claim_ledger.fractional_ledger_account.bytes(),
            &fractional_ledger_before_id.bytes(),
            &fractional_ledger_after_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &consumed_sequence.to_le_bytes(),
            &[kind],
            &[outcome],
            &amount.to_le_bytes(),
        ],
    );
    transition_id.require_live()?;
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        aggregate_materialized_supply,
        next_fractional_sequence,
        last_fractional_transition_id: transition_id,
        ..claim_ledger
    };
    claim_ledger_after.validate()?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;
    if claim_ledger_after_id == claim_ledger_before_id {
        return Err(Error::MismatchedBinding);
    }
    Ok(FractionalClaimLedgerPlanV3 {
        claim_ledger_before_id,
        fractional_ledger_before_id,
        claim_ledger_after,
        claim_ledger_after_id,
        fractional_ledger_after_id,
        transition_id,
        consumed_sequence,
    })
}

/// ClaimLedger half of one explicitly absent→live 0xa5 founding transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimLedgerFoundingPlanV3 {
    claim_ledger_before_id: Id,
    fractional_ledger_account: Id,
    fractional_ledger_after_id: Id,
    claim_ledger_after: ClaimLedgerV3,
    claim_ledger_after_id: Id,
    transition_id: Id,
}

impl FractionalClaimLedgerFoundingPlanV3 {
    /// Founding ClaimLedger semantic ID before the latch is consumed.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.claim_ledger_before_id
    }

    /// Exact canonical 0xa5 account whose absence must be authenticated.
    pub const fn fractional_ledger_account(self) -> Id {
        self.fractional_ledger_account
    }

    /// Exact newly created 0xa5 semantic ID.
    pub const fn fractional_ledger_after_id(self) -> Id {
        self.fractional_ledger_after_id
    }

    /// Complete ClaimLedger successor at next fractional sequence one.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.claim_ledger_after
    }

    /// ClaimLedger successor semantic ID.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.claim_ledger_after_id
    }

    /// Shared founding transition identity.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }
}

/// Consume the unique sequence-zero founding latch after the SBF adapter has
/// authenticated the exact 0xa5 PDA's absence and its atomic creation plan.
pub fn prepare_fractional_claim_ledger_founding_v3<B: PositionV3Sha256Backend>(
    claim_ledger: ClaimLedgerV3,
    fractional_ledger_after_id: Id,
    backend: &B,
) -> Result<FractionalClaimLedgerFoundingPlanV3> {
    claim_ledger.validate()?;
    fractional_ledger_after_id.require_live()?;
    if claim_ledger.next_fractional_sequence != 0
        || !claim_ledger.last_fractional_transition_id.is_zero()
    {
        return Err(Error::MismatchedBinding);
    }
    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let transition_id = digest(
        FRACTIONAL_CLAIM_LEDGER_FOUNDING_DOMAIN_V3,
        &[
            &[0],
            &claim_ledger.market_instance_id.bytes(),
            &claim_ledger.fractional_policy_id.bytes(),
            &claim_ledger.fractional_ledger_account.bytes(),
            &fractional_ledger_after_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &0u64.to_le_bytes(),
        ],
    );
    transition_id.require_live()?;
    let claim_ledger_after = ClaimLedgerV3 {
        next_fractional_sequence: 1,
        last_fractional_transition_id: transition_id,
        ..claim_ledger
    };
    claim_ledger_after.validate()?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;
    Ok(FractionalClaimLedgerFoundingPlanV3 {
        claim_ledger_before_id,
        fractional_ledger_account: claim_ledger.fractional_ledger_account,
        fractional_ledger_after_id,
        claim_ledger_after,
        claim_ledger_after_id,
        transition_id,
    })
}

/// Exact exhausted ClaimLedger successor paired with the final live 0xa5
/// retirement successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimLedgerRetirementPlanV3 {
    claim_ledger_before_id: Id,
    fractional_ledger_before_id: Id,
    fractional_ledger_retirement_id: Id,
    claim_ledger_after: ClaimLedgerV3,
    claim_ledger_after_id: Id,
    transition_id: Id,
    consumed_sequence: u64,
}

impl FractionalClaimLedgerRetirementPlanV3 {
    /// Resolved ClaimLedger semantic ID before terminalization.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.claim_ledger_before_id
    }

    /// Final live 0xa5 semantic ID before its retirement successor.
    pub const fn fractional_ledger_before_id(self) -> Id {
        self.fractional_ledger_before_id
    }

    /// Exact 0xa5 retirement successor semantic ID.
    pub const fn fractional_ledger_retirement_id(self) -> Id {
        self.fractional_ledger_retirement_id
    }

    /// Complete ClaimLedger successor in Retiring lifecycle.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.claim_ledger_after
    }

    /// Retiring ClaimLedger semantic ID.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.claim_ledger_after_id
    }

    /// Shared terminal transition identity retained as the last latch.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }

    /// Exact consumed fractional sequence.
    pub const fn consumed_sequence(self) -> u64 {
        self.consumed_sequence
    }
}

/// Move an exhausted resolved ClaimLedger to Retiring while committing the
/// exact final 0xa5 successor. K and live-credit count remain authenticated by
/// the Fractional owner; the SBF composer must prove both are zero before it
/// may consume this pure plan and delete 0xa5.
pub fn prepare_fractional_claim_ledger_retirement_v3<B: PositionV3Sha256Backend>(
    claim_ledger: ClaimLedgerV3,
    fractional_ledger_before_id: Id,
    fractional_ledger_retirement_id: Id,
    consumed_sequence: u64,
    backend: &B,
) -> Result<FractionalClaimLedgerRetirementPlanV3> {
    claim_ledger.validate()?;
    fractional_ledger_before_id.require_live()?;
    fractional_ledger_retirement_id.require_live()?;
    if claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.next_fractional_sequence != consumed_sequence
        || fractional_ledger_before_id == fractional_ledger_retirement_id
    {
        return Err(Error::MismatchedBinding);
    }
    let mut index = 0usize;
    while index < usize::from(claim_ledger.outcome_count) {
        if claim_ledger.aggregate_internal_supply[index] != 0
            || claim_ledger.aggregate_materialized_supply[index] != 0
        {
            return Err(Error::AggregateLiabilityInsufficient);
        }
        index += 1;
    }
    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let next_fractional_sequence = consumed_sequence.checked_add(1).ok_or(Error::Arithmetic)?;
    let transition_id = digest(
        FRACTIONAL_CLAIM_LEDGER_RETIREMENT_DOMAIN_V3,
        &[
            &claim_ledger.market_instance_id.bytes(),
            &claim_ledger.fractional_policy_id.bytes(),
            &claim_ledger.fractional_ledger_account.bytes(),
            &fractional_ledger_before_id.bytes(),
            &fractional_ledger_retirement_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &consumed_sequence.to_le_bytes(),
            &[MarketLiabilityLifecycleV1::Retiring as u8],
        ],
    );
    transition_id.require_live()?;
    let claim_ledger_after = ClaimLedgerV3 {
        lifecycle: MarketLiabilityLifecycleV1::Retiring,
        next_fractional_sequence,
        last_fractional_transition_id: transition_id,
        ..claim_ledger
    };
    claim_ledger_after.validate()?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;
    Ok(FractionalClaimLedgerRetirementPlanV3 {
        claim_ledger_before_id,
        fractional_ledger_before_id,
        fractional_ledger_retirement_id,
        claim_ledger_after,
        claim_ledger_after_id,
        transition_id,
        consumed_sequence,
    })
}

/// Atomic Hoard/ClaimLedger half of one fractional redemption or credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimRedemptionPlanV3 {
    fractional: FractionalClaimLedgerPlanV3,
    hoard_before_id: Id,
    hoard_after: HoardV2,
    hoard_after_id: Id,
    payout_atoms: u64,
    disposition: FractionalPayoutDispositionV3,
    receipt_id: Id,
}

impl FractionalClaimRedemptionPlanV3 {
    /// Exact latched ClaimLedger/0xa5 successor.
    pub const fn fractional(self) -> FractionalClaimLedgerPlanV3 {
        self.fractional
    }

    /// Hoard semantic ID before releasing locked principal.
    pub const fn hoard_before_id(self) -> Id {
        self.hoard_before_id
    }

    /// Complete permitted Hoard successor.
    pub const fn hoard_after(self) -> HoardV2 {
        self.hoard_after
    }

    /// Hoard semantic ID after releasing locked principal.
    pub const fn hoard_after_id(self) -> Id {
        self.hoard_after_id
    }

    /// Exact whole collateral atoms paid; zero is canonical for credit-only work.
    pub const fn payout_atoms(self) -> u64 {
        self.payout_atoms
    }

    /// Whether paid atoms become Position cash or leave token custody.
    pub const fn disposition(self) -> FractionalPayoutDispositionV3 {
        self.disposition
    }

    /// Canonical atomic redemption receipt.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

/// Custody classification of whole atoms paid by fractional redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalPayoutDispositionV3 {
    /// Locked principal becomes canonical PositionV3 cash and stays in Hoard.
    InternalPositionCash,
    /// Locked principal leaves Hoard through an accepted Realm-selected CPI.
    /// `None` is canonical only for a zero payout, which emits no CPI.
    ExternalCustodyTransfer {
        /// Exact accepted claim-redemption custody movement when nonzero.
        accepted: Option<AcceptedClaimRedemptionCollateralV2>,
    },
}

/// Burn or preserve native supply, advance the 0xa5 latch, and release exactly
/// the paid whole atoms from locked Hoard principal in one typed plan.
#[allow(clippy::too_many_arguments)]
pub fn prepare_fractional_claim_redemption_v3<B: PositionV3Sha256Backend>(
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    fractional_ledger_before_id: Id,
    fractional_ledger_after_id: Id,
    consumed_sequence: u64,
    mutation: FractionalClaimSupplyMutationV3,
    payout_atoms: u64,
    disposition: FractionalPayoutDispositionV3,
    backend: &B,
) -> Result<FractionalClaimRedemptionPlanV3> {
    hoard.validate()?;
    claim_ledger.validate()?;
    if hoard.market_instance_id != claim_ledger.market_instance_id
        || hoard.realm_id != claim_ledger.realm_id
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || hoard.outcome_count != claim_ledger.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let fractional = prepare_fractional_claim_ledger_successor_v3(
        claim_ledger,
        fractional_ledger_before_id,
        fractional_ledger_after_id,
        consumed_sequence,
        mutation,
        backend,
    )?;
    let hoard_before_id = hoard.semantic_id(backend)?;
    let locked_claim_principal_atoms = hoard
        .locked_claim_principal_atoms
        .checked_sub(payout_atoms)
        .ok_or(Error::AggregateLiabilityInsufficient)?;
    let (cash_liability_atoms, custody_receipt_id) = match disposition {
        FractionalPayoutDispositionV3::InternalPositionCash => (
            hoard
                .cash_liability_atoms
                .checked_add(payout_atoms)
                .ok_or(Error::Arithmetic)?,
            Id::ZERO,
        ),
        FractionalPayoutDispositionV3::ExternalCustodyTransfer { accepted } => {
            match (payout_atoms, accepted) {
                (0, None) => (hoard.cash_liability_atoms, Id::ZERO),
                (0, Some(_)) | (_, None) => return Err(Error::MismatchedBinding),
                (amount, Some(accepted)) => {
                    let custody = accepted.custody();
                    let request = accepted.request();
                    let backing_after = accepted.backing_after();
                    let visible_hoard_after = custody
                        .hoard_atoms_after
                        .ok_or(Error::PostAdmissionFailed)?;
                    let required_custody_atoms = hoard
                        .cash_liability_atoms
                        .checked_add(locked_claim_principal_atoms)
                        .ok_or(Error::Arithmetic)?;
                    if request.claim_redemption_id != fractional.transition_id
                        || request.payout_atoms != amount
                        || custody.kind != CustodyTransferKindV2::ClaimRedemption
                        || custody.source_semantic_owner != hoard.market_instance_id
                        || custody.amount_atoms != amount
                        || backing_after.locked_atoms != locked_claim_principal_atoms
                        || backing_after.cap_atoms != hoard.collateral_cap_atoms
                        || visible_hoard_after < required_custody_atoms
                    {
                        return Err(Error::PostAdmissionFailed);
                    }
                    (hoard.cash_liability_atoms, accepted.receipt_id())
                }
            }
        }
    };
    let hoard_after = HoardV2 {
        locked_claim_principal_atoms,
        cash_liability_atoms,
        ..hoard
    };
    hoard_after.validate()?;
    let hoard_after_id = hoard_after.semantic_id(backend)?;
    if payout_atoms == 0 && hoard_after_id != hoard_before_id {
        return Err(Error::MismatchedBinding);
    }
    let receipt_id = digest(
        FRACTIONAL_CLAIM_REDEMPTION_DOMAIN_V3,
        &[
            &[match disposition {
                FractionalPayoutDispositionV3::InternalPositionCash => 1,
                FractionalPayoutDispositionV3::ExternalCustodyTransfer { .. } => 2,
            }],
            &hoard.market_instance_id.bytes(),
            &fractional.transition_id.bytes(),
            &fractional.claim_ledger_before_id.bytes(),
            &fractional.claim_ledger_after_id.bytes(),
            &hoard_before_id.bytes(),
            &hoard_after_id.bytes(),
            &payout_atoms.to_le_bytes(),
            &custody_receipt_id.bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(FractionalClaimRedemptionPlanV3 {
        fractional,
        hoard_before_id,
        hoard_after,
        hoard_after_id,
        payout_atoms,
        disposition,
        receipt_id,
    })
}

/// Direction of a Holder↔Hoard cash-liability transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoardCashLiabilityKindV2 {
    /// External collateral enters custody and Position cash increases.
    Deposit,
    /// Position cash decreases and external collateral exits custody.
    Withdrawal,
}

/// Exact classified-cash successor and receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoardCashLiabilityPlanV2 {
    /// Exact Hoard prestate semantic ID.
    pub hoard_before_id: Id,
    /// Complete permitted Hoard successor.
    pub hoard_after: HoardV2,
    /// Exact Hoard successor semantic ID.
    pub hoard_after_id: Id,
    /// Canonical transition receipt.
    pub receipt_id: Id,
}

/// Update only the aggregate Position-cash liability classification.
pub fn prepare_hoard_cash_liability_v2<B: PositionV3Sha256Backend>(
    hoard: HoardV2,
    kind: HoardCashLiabilityKindV2,
    amount_atoms: u64,
    backend: &B,
) -> Result<HoardCashLiabilityPlanV2> {
    hoard.validate()?;
    if amount_atoms == 0
        || (kind == HoardCashLiabilityKindV2::Deposit
            && hoard.lifecycle != MarketLiabilityLifecycleV1::Open)
        || hoard.lifecycle == MarketLiabilityLifecycleV1::Retiring
    {
        return Err(Error::InvalidParameter);
    }
    let hoard_before_id = hoard.semantic_id(backend)?;
    let cash_liability_atoms = match kind {
        HoardCashLiabilityKindV2::Deposit => hoard
            .cash_liability_atoms
            .checked_add(amount_atoms)
            .ok_or(Error::Arithmetic)?,
        HoardCashLiabilityKindV2::Withdrawal => hoard
            .cash_liability_atoms
            .checked_sub(amount_atoms)
            .ok_or(Error::AggregateLiabilityInsufficient)?,
    };
    let hoard_after = HoardV2 {
        cash_liability_atoms,
        ..hoard
    };
    hoard_after.validate()?;
    let hoard_after_id = hoard_after.semantic_id(backend)?;
    let kind_byte = [match kind {
        HoardCashLiabilityKindV2::Deposit => 1,
        HoardCashLiabilityKindV2::Withdrawal => 2,
    }];
    let receipt_id = digest(
        HOARD_CASH_LIABILITY_RECEIPT_DOMAIN_V2,
        &[
            &kind_byte,
            &hoard.market_instance_id.bytes(),
            &hoard_before_id.bytes(),
            &hoard_after_id.bytes(),
            &amount_atoms.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(HoardCashLiabilityPlanV2 {
        hoard_before_id,
        hoard_after,
        hoard_after_id,
        receipt_id,
    })
}

/// Accepted external custody delta joined to both canonical liability owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedPositionHoardCashTransitionV3 {
    position: AcceptedPositionCollateralTransferV3,
    hoard: HoardCashLiabilityPlanV2,
    transition_id: Id,
}

impl AcceptedPositionHoardCashTransitionV3 {
    /// Exact accepted Position/custody successor.
    pub const fn position(self) -> AcceptedPositionCollateralTransferV3 {
        self.position
    }

    /// Exact classified Hoard successor.
    pub const fn hoard(self) -> HoardCashLiabilityPlanV2 {
        self.hoard
    }

    /// Canonical identity committed by GEN1 Replay.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }
}

/// Join an accepted Holder↔Hoard CPI to the exact aggregate cash-liability
/// successor and require visible custody to cover cash plus locked principal.
pub fn accept_position_hoard_cash_transition_v3<B: PositionV3Sha256Backend>(
    position: AcceptedPositionCollateralTransferV3,
    hoard: HoardV2,
    backend: &B,
) -> Result<AcceptedPositionHoardCashTransitionV3> {
    hoard.validate()?;
    let kind = match position.kind() {
        CustodyTransferKindV2::HolderDeposit => HoardCashLiabilityKindV2::Deposit,
        CustodyTransferKindV2::HolderWithdrawal => HoardCashLiabilityKindV2::Withdrawal,
        _ => return Err(Error::InvalidParameter),
    };
    let position_post = position.position_post();
    if Id::from_bytes(position_post.market_instance_id().bytes()) != hoard.market_instance_id
        || Id::from_bytes(position_post.realm_id().bytes()) != hoard.realm_id
        || Id::from_bytes(position_post.collateral_policy_id().bytes())
            != hoard.collateral_policy_id
        || Id::from_bytes(position_post.collateral_release_id().bytes())
            != hoard.collateral_release_id
    {
        return Err(Error::MismatchedBinding);
    }
    let hoard_plan =
        prepare_hoard_cash_liability_v2(hoard, kind, position.amount_atoms(), backend)?;
    if position.hoard_atoms_after() < hoard_plan.hoard_after.required_custody_atoms()? {
        return Err(Error::HoardCoverageMismatch);
    }
    let custody_receipt = position.receipt_id()?;
    let transition_id = digest(
        POSITION_HOARD_CASH_TRANSITION_DOMAIN_V3,
        &[
            &position.position_account_id().bytes(),
            &position.position_pre_semantic_id().bytes(),
            &position.position_post_semantic_id().bytes(),
            &hoard_plan.hoard_before_id.bytes(),
            &hoard_plan.hoard_after_id.bytes(),
            &custody_receipt.bytes(),
            &hoard_plan.receipt_id.bytes(),
        ],
    );
    transition_id.require_live()?;
    Ok(AcceptedPositionHoardCashTransitionV3 {
        position,
        hoard: hoard_plan,
        transition_id,
    })
}

/// Split locks a complete set; Merge unlocks one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteSetReclassificationKindV3 {
    /// Cash liability becomes locked claim principal and native claims appear.
    Split,
    /// Native claims disappear and locked principal becomes cash liability.
    Merge,
}

/// Atomic Hoard/ClaimLedger successor for one complete-set reclassification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteSetReclassificationPlanV3 {
    /// Exact Hoard prestate semantic ID.
    pub hoard_before_id: Id,
    /// Exact ClaimLedger prestate semantic ID.
    pub claim_ledger_before_id: Id,
    /// Complete permitted Hoard successor.
    pub hoard_after: HoardV2,
    /// Complete permitted ClaimLedger successor.
    pub claim_ledger_after: ClaimLedgerV3,
    /// Exact Hoard successor semantic ID.
    pub hoard_after_id: Id,
    /// Exact ClaimLedger successor semantic ID.
    pub claim_ledger_after_id: Id,
    /// Canonical transition receipt.
    pub receipt_id: Id,
}

/// Reclassify cash/locked custody and every active native outcome atomically.
pub fn prepare_complete_set_reclassification_v3<B: PositionV3Sha256Backend>(
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    kind: CompleteSetReclassificationKindV3,
    quantity: u64,
    backend: &B,
) -> Result<CompleteSetReclassificationPlanV3> {
    hoard.validate()?;
    claim_ledger.validate()?;
    if quantity == 0
        || hoard.market_instance_id != claim_ledger.market_instance_id
        || hoard.realm_id != claim_ledger.realm_id
        || hoard.lifecycle != claim_ledger.lifecycle
        || hoard.outcome_count != claim_ledger.outcome_count
        || hoard.lifecycle == MarketLiabilityLifecycleV1::Retiring
        || (kind == CompleteSetReclassificationKindV3::Split
            && hoard.lifecycle != MarketLiabilityLifecycleV1::Open)
    {
        return Err(Error::MismatchedBinding);
    }
    let hoard_before_id = hoard.semantic_id(backend)?;
    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let (cash_liability_atoms, locked_claim_principal_atoms) = match kind {
        CompleteSetReclassificationKindV3::Split => (
            hoard
                .cash_liability_atoms
                .checked_sub(quantity)
                .ok_or(Error::AggregateLiabilityInsufficient)?,
            hoard
                .locked_claim_principal_atoms
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?,
        ),
        CompleteSetReclassificationKindV3::Merge => (
            hoard
                .cash_liability_atoms
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?,
            hoard
                .locked_claim_principal_atoms
                .checked_sub(quantity)
                .ok_or(Error::AggregateLiabilityInsufficient)?,
        ),
    };
    let hoard_after = HoardV2 {
        cash_liability_atoms,
        locked_claim_principal_atoms,
        ..hoard
    };
    let mut aggregate_internal_supply = claim_ledger.aggregate_internal_supply;
    let mut index = 0usize;
    while index < usize::from(claim_ledger.outcome_count) {
        aggregate_internal_supply[index] = match kind {
            CompleteSetReclassificationKindV3::Split => aggregate_internal_supply[index]
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?,
            CompleteSetReclassificationKindV3::Merge => aggregate_internal_supply[index]
                .checked_sub(quantity)
                .ok_or(Error::AggregateLiabilityInsufficient)?,
        };
        index += 1;
    }
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        ..claim_ledger
    };
    hoard_after.validate()?;
    claim_ledger_after.validate()?;
    let hoard_after_id = hoard_after.semantic_id(backend)?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;
    let kind_byte = [match kind {
        CompleteSetReclassificationKindV3::Split => 1,
        CompleteSetReclassificationKindV3::Merge => 2,
    }];
    let receipt_id = digest(
        COMPLETE_SET_RECLASSIFICATION_RECEIPT_DOMAIN_V3,
        &[
            &kind_byte,
            &hoard.market_instance_id.bytes(),
            &hoard_before_id.bytes(),
            &hoard_after_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &claim_ledger_after_id.bytes(),
            &quantity.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(CompleteSetReclassificationPlanV3 {
        hoard_before_id,
        claim_ledger_before_id,
        hoard_after,
        claim_ledger_after,
        hoard_after_id,
        claim_ledger_after_id,
        receipt_id,
    })
}

fn write_header(
    writer: &mut Writer<'_>,
    tag: u8,
    version: u8,
    lifecycle: MarketLiabilityLifecycleV1,
    outcome_count: u8,
    stored_bump: u8,
) -> Result<()> {
    writer.u8(tag)?;
    writer.u8(version)?;
    writer.u8(lifecycle as u8)?;
    writer.u8(outcome_count)?;
    writer.u8(stored_bump)?;
    writer.u8(0)?;
    writer.bytes(&[0; 10])
}

fn read_header(
    reader: &mut Reader<'_>,
    expected_tag: u8,
    expected_version: u8,
) -> Result<(MarketLiabilityLifecycleV1, u8, u8)> {
    if reader.u8()? != expected_tag {
        return Err(Error::BadMagic);
    }
    if reader.u8()? != expected_version {
        return Err(Error::BadVersion);
    }
    let lifecycle = MarketLiabilityLifecycleV1::decode(reader.u8()?)?;
    let outcome_count = reader.u8()?;
    let stored_bump = reader.u8()?;
    if reader.u8()? != 0 {
        return Err(Error::NonCanonicalPadding);
    }
    reader.require_zeroes(10)?;
    Ok((lifecycle, outcome_count, stored_bump))
}

#[allow(dead_code)]
fn rent_payer_id(rent: DeletableRentOwnerV1) -> Id {
    Id::from_bytes(rent.payer().bytes())
}

#[allow(dead_code)]
fn identity(value: Id) -> Result<Identity32V1> {
    Identity32V1::new(value.bytes()).map_err(|_| Error::ZeroIdentity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bind_collateral_profile_v2, AdapterCatalogV2, AdapterReleaseV2, CollateralPolicyV2,
        MarketCollateralBindingV2, ProfileCollateralBindingV2, RealmCollateralBindingV2,
        RuntimeReleaseObservationV2, LEGACY_SPL_TOKEN_PROGRAM,
    };
    use sha2::{Digest, Sha256};

    static RELEASES: [AdapterReleaseV2; 1] = [AdapterReleaseV2::legacy_spl(
        Id::from_bytes([20; 32]),
        Id::from_bytes([21; 32]),
    )];

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestSha256;

    impl PositionV3Sha256Backend for TestSha256 {
        fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update(domain);
            hasher.update(body);
            hasher.finalize().into()
        }
    }

    const fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn rent(payer: u8, principal: u64) -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1::from_persisted(Identity32V1::new([payer; 32]).unwrap(), principal, 0)
            .unwrap()
    }

    fn bound() -> crate::BoundCollateralProfileV2 {
        let release = RELEASES[0];
        let policy =
            CollateralPolicyV2::for_release(release, id(22), 6, 1_000, 500, 0, 0, 0, 0).unwrap();
        let policy_id = policy.id().unwrap();
        bind_collateral_profile_v2(
            MarketCollateralBindingV2 {
                market: id(3),
                realm: id(1),
                profile: id(2),
                collateral_cap_atoms: 500,
                hoard_authority: id(4),
                hoard_token_account: id(5),
            },
            RealmCollateralBindingV2 {
                realm: id(1),
                profile: id(2),
            },
            ProfileCollateralBindingV2 {
                profile: id(2),
                collateral_policy: policy_id,
                adapter_release: release.id().unwrap(),
            },
            policy,
            AdapterCatalogV2::new(&RELEASES).unwrap(),
            RuntimeReleaseObservationV2 {
                token_program: LEGACY_SPL_TOKEN_PROGRAM,
                token_program_executable: true,
                token_program_writable: false,
                token_program_signer: false,
                token_program_deployment: id(20),
                parser_cpi_code: id(21),
            },
        )
        .unwrap()
    }

    fn request() -> MarketLiabilityFoundingRequestV3 {
        MarketLiabilityFoundingRequestV3 {
            hoard_account: id(6),
            claim_ledger_account: id(7),
            market_instance_id: id(3),
            native_claim_basis_id: id(8),
            fractional_policy_id: id(9),
            fractional_ledger_account: id(10),
            claim_mint_authority: id(11),
            outcome_count: 2,
            hoard_bump: 12,
            claim_ledger_bump: 13,
            hoard_rent: rent(14, 100),
            claim_ledger_rent: rent(15, 200),
        }
    }

    #[test]
    fn founding_plan_starts_every_liability_and_supply_at_zero() {
        let plan = prepare_market_liability_founding_v3(bound(), request(), &TestSha256).unwrap();
        assert_eq!(plan.hoard_account(), id(6));
        assert_eq!(plan.claim_ledger_account(), id(7));
        assert_eq!(plan.claim_mint_authority(), id(11));
        assert_eq!(plan.hoard().cash_liability_atoms, 0);
        assert_eq!(plan.hoard().locked_claim_principal_atoms, 0);
        assert_eq!(
            plan.claim_ledger().aggregate_internal_supply,
            [0; MAX_OUTCOMES]
        );
        assert_eq!(
            plan.claim_ledger().aggregate_materialized_supply,
            [0; MAX_OUTCOMES]
        );
        assert_eq!(plan.claim_ledger().resolution_account, Id::ZERO);
        assert_eq!(plan.claim_ledger().next_fractional_sequence, 0);
        assert_eq!(plan.claim_ledger().last_fractional_transition_id, Id::ZERO);
        assert!(!plan.founding_id().is_zero());
    }

    #[test]
    fn founding_plan_refuses_a_market_substitution_or_invalid_width() {
        let mut wrong_market = request();
        wrong_market.market_instance_id = id(30);
        assert_eq!(
            prepare_market_liability_founding_v3(bound(), wrong_market, &TestSha256),
            Err(Error::MismatchedBinding)
        );

        let mut zero_outcomes = request();
        zero_outcomes.outcome_count = 0;
        assert_eq!(
            prepare_market_liability_founding_v3(bound(), zero_outcomes, &TestSha256),
            Err(Error::MismatchedBinding)
        );
    }
}
