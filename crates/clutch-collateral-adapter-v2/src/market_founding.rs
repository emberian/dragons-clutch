// SPDX-License-Identifier: AGPL-3.0-or-later

//! Closed composition of the full-width Market collateral and claim planes.
//!
//! Product owns whether a Market may be founded and General owns the immutable
//! MarketBinding/MarketRuntime poststate. This module owns neither authority.
//! It composes their selected MarketRuntime mint authority with the separately
//! authenticated collateral and claim releases so the runtime cannot create a
//! mixture of individually valid but mutually unrelated accounts.

use crate::{
    digest, AcceptedClaimMintFoundingStepV2, AcceptedMarketLiabilityFoundingV3,
    AcceptedOutcomeCustodyFoundingStepV1, BoundCollateralProfileV2, ClaimMintFoundingPlanV2,
    CustodyCreationPlanV2, Error, Id, MarketLiabilityFoundingPlanV3,
    OutcomeCustodyFoundingPlanV1, Result,
};
use clutch_retirement::MAX_OUTCOMES;

/// Earlier Market-core founding identity without outcome-custody ownership.
pub const MARKET_CORE_FOUNDING_DOMAIN_V3: &[u8] = b"dragons-clutch/market-core/founding/v3\0";
/// Earlier accepted Market-core receipt without outcome-custody ownership.
pub const ACCEPTED_MARKET_CORE_FOUNDING_DOMAIN_V3: &[u8] =
    b"dragons-clutch/market-core/founding-accepted/v3\0";

/// Earlier collateral/claim founding plan. This remains decode/source
/// compatible but is not sufficient for the current 47-slot Product graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketCoreFoundingPlanV3 {
    liabilities: MarketLiabilityFoundingPlanV3,
    custody: CustodyCreationPlanV2,
    claim_mints: ClaimMintFoundingPlanV2,
    core_id: Id,
}

/// Earlier accepted Market-core capability without per-outcome custodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedMarketCoreFoundingV3 {
    plan: MarketCoreFoundingPlanV3,
    liability_receipt_id: Id,
    mint_receipt_ids: [Id; MAX_OUTCOMES],
    receipt_id: Id,
}

impl AcceptedMarketCoreFoundingV3 {
    pub const fn plan(self) -> MarketCoreFoundingPlanV3 { self.plan }
    pub const fn liability_receipt_id(self) -> Id { self.liability_receipt_id }
    pub const fn mint_receipt_ids(self) -> [Id; MAX_OUTCOMES] { self.mint_receipt_ids }
    pub const fn receipt_id(self) -> Id { self.receipt_id }
}

impl MarketCoreFoundingPlanV3 {
    pub const fn liabilities(self) -> MarketLiabilityFoundingPlanV3 { self.liabilities }
    pub const fn custody(self) -> CustodyCreationPlanV2 { self.custody }
    pub const fn claim_mints(self) -> ClaimMintFoundingPlanV2 { self.claim_mints }
    pub const fn core_id(self) -> Id { self.core_id }
}

/// Compose the preserved pre-outcome-custody Market core. Current Product
/// activation must use V4 below.
pub fn compose_market_core_founding_v3(
    bound: BoundCollateralProfileV2,
    liabilities: MarketLiabilityFoundingPlanV3,
    custody: CustodyCreationPlanV2,
    claim_mints: ClaimMintFoundingPlanV2,
) -> Result<MarketCoreFoundingPlanV3> {
    let market = bound.market();
    let realm = bound.realm_bound().realm();
    let policy = bound.policy();
    let release = bound.release();
    let hoard = liabilities.hoard();
    let claim_ledger = liabilities.claim_ledger();

    if hoard.market_instance_id != market.market
        || hoard.realm_id != realm.realm
        || hoard.profile_id != realm.profile
        || hoard.collateral_policy_id != bound.policy_id()
        || hoard.collateral_release_id != release.id()?
        || hoard.authority != market.hoard_authority
        || hoard.token_account != market.hoard_token_account
        || hoard.collateral_cap_atoms != market.collateral_cap_atoms
        || claim_ledger.market_instance_id != market.market
        || claim_ledger.realm_id != realm.realm
        || claim_ledger.outcome_count != hoard.outcome_count
        || claim_mints.market_instance_id() != market.market
        || claim_mints.mint_authority() != liabilities.claim_mint_authority()
        || claim_mints.outcome_count() != hoard.outcome_count
        || custody.token_program != release.token_program
        || custody.account != market.hoard_token_account
        || custody.owner_authority != market.hoard_authority
        || custody.mint != policy.mint
        || custody.account_bytes != release.custody_account_bytes
    {
        return Err(Error::MismatchedBinding);
    }
    for pair in [
        (liabilities.hoard_account(), liabilities.claim_ledger_account()),
        (liabilities.hoard_account(), market.hoard_authority),
        (liabilities.hoard_account(), market.hoard_token_account),
        (liabilities.claim_ledger_account(), market.hoard_authority),
        (liabilities.claim_ledger_account(), market.hoard_token_account),
        (market.hoard_authority, market.hoard_token_account),
    ] {
        if pair.0 == pair.1 {
            return Err(Error::MismatchedBinding);
        }
    }
    let mut outcome = 0u8;
    while outcome < claim_mints.outcome_count() {
        let mint = claim_mints.outcome_mint(outcome)?;
        if mint == liabilities.hoard_account()
            || mint == liabilities.claim_ledger_account()
            || mint == market.hoard_authority
            || mint == market.hoard_token_account
            || mint == policy.mint
        {
            return Err(Error::MismatchedBinding);
        }
        outcome = outcome.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let core_id = digest(
        MARKET_CORE_FOUNDING_DOMAIN_V3,
        &[
            &market.market.bytes(),
            &realm.realm.bytes(),
            &realm.profile.bytes(),
            &bound.policy_id().bytes(),
            &release.id()?.bytes(),
            &liabilities.founding_id().bytes(),
            &claim_mints.founding_id().bytes(),
            &custody.token_program.bytes(),
            &custody.account.bytes(),
            &custody.owner_authority.bytes(),
            &custody.mint.bytes(),
            &custody.account_bytes.to_le_bytes(),
        ],
    );
    core_id.require_live()?;
    Ok(MarketCoreFoundingPlanV3 { liabilities, custody, claim_mints, core_id })
}

/// Accept the preserved pre-outcome-custody Market core. Current Product
/// activation must use V4 below.
pub fn accept_market_core_founding_v3(
    plan: MarketCoreFoundingPlanV3,
    liabilities: AcceptedMarketLiabilityFoundingV3,
    mint_steps: [Option<AcceptedClaimMintFoundingStepV2>; MAX_OUTCOMES],
) -> Result<AcceptedMarketCoreFoundingV3> {
    if liabilities.plan() != plan.liabilities {
        return Err(Error::PostAdmissionFailed);
    }
    let mut mint_receipt_ids = [Id::ZERO; MAX_OUTCOMES];
    let active = usize::from(plan.claim_mints.outcome_count());
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        match (index < active, mint_steps[index]) {
            (true, Some(accepted)) => {
                let outcome = u8::try_from(index).map_err(|_| Error::Arithmetic)?;
                if accepted.step() != plan.claim_mints.step(outcome)? {
                    return Err(Error::PostAdmissionFailed);
                }
                mint_receipt_ids[index] = accepted.receipt_id();
            }
            (false, None) => {}
            _ => return Err(Error::PostAdmissionFailed),
        }
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let mut receipt_bytes = [[0u8; 32]; MAX_OUTCOMES];
    index = 0;
    while index < MAX_OUTCOMES {
        receipt_bytes[index] = mint_receipt_ids[index].bytes();
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let outcome_count = [plan.claim_mints.outcome_count()];
    let mut parts: [&[u8]; 4 + MAX_OUTCOMES] = [&[]; 4 + MAX_OUTCOMES];
    let core_bytes = plan.core_id.bytes();
    let liability_bytes = liabilities.receipt_id().bytes();
    let mint_founding_bytes = plan.claim_mints.founding_id().bytes();
    parts[0] = &core_bytes;
    parts[1] = &liability_bytes;
    parts[2] = &mint_founding_bytes;
    parts[3] = &outcome_count;
    index = 0;
    while index < MAX_OUTCOMES {
        parts[4 + index] = &receipt_bytes[index];
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let receipt_id = digest(ACCEPTED_MARKET_CORE_FOUNDING_DOMAIN_V3, &parts);
    receipt_id.require_live()?;
    Ok(AcceptedMarketCoreFoundingV3 {
        plan,
        liability_receipt_id: liabilities.receipt_id(),
        mint_receipt_ids,
        receipt_id,
    })
}

/// Full-width shared Market-core founding identity domain.
pub const MARKET_CORE_FOUNDING_DOMAIN_V4: &[u8] = b"dragons-clutch/market-core/founding/v4\0";
/// Exhaustive accepted shared Market-core founding receipt domain.
pub const ACCEPTED_MARKET_CORE_FOUNDING_DOMAIN_V4: &[u8] =
    b"dragons-clutch/market-core/founding-accepted/v4\0";

/// Complete collateral/claim founding plan below Product and General
/// authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketCoreFoundingPlanV4 {
    liabilities: MarketLiabilityFoundingPlanV3,
    custody: CustodyCreationPlanV2,
    claim_mints: ClaimMintFoundingPlanV2,
    outcome_custodies: OutcomeCustodyFoundingPlanV1,
    core_id: Id,
}

/// Exhaustive accepted Market-core founding capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedMarketCoreFoundingV4 {
    plan: MarketCoreFoundingPlanV4,
    liability_receipt_id: Id,
    mint_receipt_ids: [Id; MAX_OUTCOMES],
    custody_receipt_ids: [Id; MAX_OUTCOMES],
    receipt_id: Id,
}

impl AcceptedMarketCoreFoundingV4 {
    /// Complete cross-release Market-core plan.
    pub const fn plan(self) -> MarketCoreFoundingPlanV4 {
        self.plan
    }

    /// Exact accepted HoardV2/ClaimLedgerV3/custody receipt.
    pub const fn liability_receipt_id(self) -> Id {
        self.liability_receipt_id
    }

    /// Ordered mint receipts with a canonical zero inactive tail.
    pub const fn mint_receipt_ids(self) -> [Id; MAX_OUTCOMES] {
        self.mint_receipt_ids
    }

    /// Ordered custody receipts with a canonical zero inactive tail.
    pub const fn custody_receipt_ids(self) -> [Id; MAX_OUTCOMES] {
        self.custody_receipt_ids
    }

    /// Exact exhaustive receipt Product may join before activation.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

impl MarketCoreFoundingPlanV4 {
    /// Exact HoardV2/ClaimLedgerV3 zero-state plan.
    pub const fn liabilities(self) -> MarketLiabilityFoundingPlanV3 {
        self.liabilities
    }

    /// Realm-release-selected Hoard token-account creation plan.
    pub const fn custody(self) -> CustodyCreationPlanV2 {
        self.custody
    }

    /// Independent claim-release OutcomeMintV2 plan.
    pub const fn claim_mints(self) -> ClaimMintFoundingPlanV2 {
        self.claim_mints
    }

    /// Release-selected per-outcome custody plan.
    pub const fn outcome_custodies(self) -> OutcomeCustodyFoundingPlanV1 {
        self.outcome_custodies
    }

    /// Shared identity Product can retain across bounded founding steps.
    pub const fn core_id(self) -> Id {
        self.core_id
    }
}

/// Compose exactly one mutually consistent full-width Market core.
///
/// The private-field component plans prove both release selections have
/// already run. This join checks every shared Market, Realm, authority,
/// custody, mint, and outcome-width fact. It does not prove account absence,
/// debit a FoundationVault, create an account, or activate Product state.
pub fn compose_market_core_founding_v4(
    bound: BoundCollateralProfileV2,
    liabilities: MarketLiabilityFoundingPlanV3,
    custody: CustodyCreationPlanV2,
    claim_mints: ClaimMintFoundingPlanV2,
    outcome_custodies: OutcomeCustodyFoundingPlanV1,
) -> Result<MarketCoreFoundingPlanV4> {
    let market = bound.market();
    let realm = bound.realm_bound().realm();
    let policy = bound.policy();
    let release = bound.release();
    let hoard = liabilities.hoard();
    let claim_ledger = liabilities.claim_ledger();

    if hoard.market_instance_id != market.market
        || hoard.realm_id != realm.realm
        || hoard.profile_id != realm.profile
        || hoard.collateral_policy_id != bound.policy_id()
        || hoard.collateral_release_id != release.id()?
        || hoard.authority != market.hoard_authority
        || hoard.token_account != market.hoard_token_account
        || hoard.collateral_cap_atoms != market.collateral_cap_atoms
        || claim_ledger.market_instance_id != market.market
        || claim_ledger.realm_id != realm.realm
        || claim_ledger.outcome_count != hoard.outcome_count
        || claim_mints.market_instance_id() != market.market
        || claim_mints.mint_authority() != liabilities.claim_mint_authority()
        || claim_mints.outcome_count() != hoard.outcome_count
        || outcome_custodies.market_instance_id() != market.market
        || outcome_custodies.owner_authority() != liabilities.claim_mint_authority()
        || outcome_custodies.outcome_count() != hoard.outcome_count
        || custody.token_program != release.token_program
        || custody.account != market.hoard_token_account
        || custody.owner_authority != market.hoard_authority
        || custody.mint != policy.mint
        || custody.account_bytes != release.custody_account_bytes
    {
        return Err(Error::MismatchedBinding);
    }

    for pair in [
        (
            liabilities.hoard_account(),
            liabilities.claim_ledger_account(),
        ),
        (liabilities.hoard_account(), market.hoard_authority),
        (liabilities.hoard_account(), market.hoard_token_account),
        (liabilities.claim_ledger_account(), market.hoard_authority),
        (
            liabilities.claim_ledger_account(),
            market.hoard_token_account,
        ),
        (market.hoard_authority, market.hoard_token_account),
    ] {
        if pair.0 == pair.1 {
            return Err(Error::MismatchedBinding);
        }
    }
    let mut outcome = 0u8;
    while outcome < claim_mints.outcome_count() {
        let mint = claim_mints.outcome_mint(outcome)?;
        let custody_account = outcome_custodies.outcome_custody(outcome)?;
        if mint == liabilities.hoard_account()
            || mint == liabilities.claim_ledger_account()
            || mint == market.hoard_authority
            || mint == market.hoard_token_account
            || mint == policy.mint
            || outcome_custodies.outcome_mint(outcome)? != mint
            || custody_account == liabilities.hoard_account()
            || custody_account == liabilities.claim_ledger_account()
            || custody_account == market.hoard_authority
            || custody_account == market.hoard_token_account
            || custody_account == policy.mint
        {
            return Err(Error::MismatchedBinding);
        }
        outcome = outcome.checked_add(1).ok_or(Error::Arithmetic)?;
    }

    let core_id = digest(
        MARKET_CORE_FOUNDING_DOMAIN_V4,
        &[
            &market.market.bytes(),
            &realm.realm.bytes(),
            &realm.profile.bytes(),
            &bound.policy_id().bytes(),
            &release.id()?.bytes(),
            &liabilities.founding_id().bytes(),
            &claim_mints.founding_id().bytes(),
            &outcome_custodies.founding_id().bytes(),
            &custody.token_program.bytes(),
            &custody.account.bytes(),
            &custody.owner_authority.bytes(),
            &custody.mint.bytes(),
            &custody.account_bytes.to_le_bytes(),
        ],
    );
    core_id.require_live()?;
    Ok(MarketCoreFoundingPlanV4 {
        liabilities,
        custody,
        claim_mints,
        outcome_custodies,
        core_id,
    })
}

/// Join the liability receipt and every active one-mint receipt into the only
/// complete Market-core activation capability.
///
/// Product remains the lifecycle owner and must additionally prove its exact
/// ordered Founding counter, FoundationVault debits, General founding
/// capability, and private one-shot activation authority.
pub fn accept_market_core_founding_v4(
    plan: MarketCoreFoundingPlanV4,
    liabilities: AcceptedMarketLiabilityFoundingV3,
    mint_steps: [Option<AcceptedClaimMintFoundingStepV2>; MAX_OUTCOMES],
    custody_steps: [Option<AcceptedOutcomeCustodyFoundingStepV1>; MAX_OUTCOMES],
) -> Result<AcceptedMarketCoreFoundingV4> {
    if liabilities.plan() != plan.liabilities {
        return Err(Error::PostAdmissionFailed);
    }
    let mut mint_receipt_ids = [Id::ZERO; MAX_OUTCOMES];
    let mut custody_receipt_ids = [Id::ZERO; MAX_OUTCOMES];
    let active = usize::from(plan.claim_mints.outcome_count());
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        match (index < active, mint_steps[index], custody_steps[index]) {
            (true, Some(accepted), Some(accepted_custody)) => {
                let outcome = u8::try_from(index).map_err(|_| Error::Arithmetic)?;
                if accepted.step() != plan.claim_mints.step(outcome)?
                    || accepted_custody.step() != plan.outcome_custodies.step(outcome)?
                {
                    return Err(Error::PostAdmissionFailed);
                }
                mint_receipt_ids[index] = accepted.receipt_id();
                custody_receipt_ids[index] = accepted_custody.receipt_id();
            }
            (false, None, None) => {}
            _ => return Err(Error::PostAdmissionFailed),
        }
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }

    let mut receipt_bytes = [[0u8; 32]; MAX_OUTCOMES];
    let mut custody_receipt_bytes = [[0u8; 32]; MAX_OUTCOMES];
    index = 0;
    while index < MAX_OUTCOMES {
        receipt_bytes[index] = mint_receipt_ids[index].bytes();
        custody_receipt_bytes[index] = custody_receipt_ids[index].bytes();
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let outcome_count = [plan.claim_mints.outcome_count()];
    let mut parts: [&[u8]; 5 + 2 * MAX_OUTCOMES] = [&[]; 5 + 2 * MAX_OUTCOMES];
    let core_bytes = plan.core_id.bytes();
    let liability_bytes = liabilities.receipt_id().bytes();
    let mint_founding_bytes = plan.claim_mints.founding_id().bytes();
    let custody_founding_bytes = plan.outcome_custodies.founding_id().bytes();
    parts[0] = &core_bytes;
    parts[1] = &liability_bytes;
    parts[2] = &mint_founding_bytes;
    parts[3] = &custody_founding_bytes;
    parts[4] = &outcome_count;
    index = 0;
    while index < MAX_OUTCOMES {
        parts[5 + index] = &receipt_bytes[index];
        parts[5 + MAX_OUTCOMES + index] = &custody_receipt_bytes[index];
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let receipt_id = digest(ACCEPTED_MARKET_CORE_FOUNDING_DOMAIN_V4, &parts);
    receipt_id.require_live()?;
    Ok(AcceptedMarketCoreFoundingV4 {
        plan,
        liability_receipt_id: liabilities.receipt_id(),
        mint_receipt_ids,
        custody_receipt_ids,
        receipt_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        accept_claim_mint_founding_step_v2, accept_market_liability_founding_v3,
        accept_outcome_custody_founding_step_v1, bind_claim_issuance_v1,
        bind_collateral_profile_v2, prepare_claim_mint_founding_v2, prepare_hoard_creation_v2,
        prepare_market_liability_founding_v3, prepare_outcome_custody_founding_v1,
        AdapterCatalogV2, AdapterReleaseV2, ClaimIssuanceBindingV1,
        ClaimMintFoundingPostwriteV2, ClaimMintFoundingRequestV2, ClaimRuntimeObservationV1,
        CollateralPolicyV2, MarketCollateralBindingV2, MarketLiabilityFoundingPostwriteV3,
        MarketLiabilityFoundingRequestV3, OutcomeCustodyFoundingRequestV1,
        ProfileCollateralBindingV2, RealmCollateralBindingV2, RuntimeAccountViewV2,
        RuntimeReleaseObservationV2, CLAIM_FLAGS_V1, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES,
        LEGACY_SPL_TOKEN_PROGRAM, TOKEN_2022_PROGRAM,
    };
    use clutch_retirement::{
        DeletableRentOwnerV1, Identity32V1, PositionV3Sha256Backend, MAX_OUTCOMES,
    };
    use sha2::{Digest, Sha256};

    static COLLATERAL_RELEASES: [AdapterReleaseV2; 1] = [AdapterReleaseV2::legacy_spl(
        Id::from_bytes([30; 32]),
        Id::from_bytes([31; 32]),
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

    fn rent(payer: u8) -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1::from_persisted(Identity32V1::new([payer; 32]).unwrap(), 100, 0)
            .unwrap()
    }

    fn collateral() -> BoundCollateralProfileV2 {
        let release = COLLATERAL_RELEASES[0];
        let policy =
            CollateralPolicyV2::for_release(release, id(22), 6, 1_000, 500, 0, 0, 0, 0).unwrap();
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
                collateral_policy: policy.id().unwrap(),
                adapter_release: release.id().unwrap(),
            },
            policy,
            AdapterCatalogV2::new(&COLLATERAL_RELEASES).unwrap(),
            RuntimeReleaseObservationV2 {
                token_program: LEGACY_SPL_TOKEN_PROGRAM,
                token_program_executable: true,
                token_program_writable: false,
                token_program_signer: false,
                token_program_deployment: id(30),
                parser_cpi_code: id(31),
            },
        )
        .unwrap()
    }

    fn claim() -> crate::BoundClaimIssuanceV1 {
        let binding = ClaimIssuanceBindingV1 {
            flags: CLAIM_FLAGS_V1,
            adapter_release: id(40),
            token_program: TOKEN_2022_PROGRAM,
            token_program_deployment: id(41),
            parser_cpi_code: id(42),
            decimals: 0,
            mint_extensions: 0,
            account_extensions: 0,
        };
        bind_claim_issuance_v1(
            binding.id().unwrap(),
            binding,
            ClaimRuntimeObservationV1 {
                token_program: TOKEN_2022_PROGRAM,
                token_program_executable: true,
                token_program_writable: false,
                token_program_signer: false,
                token_program_deployment: id(41),
                parser_cpi_code: id(42),
            },
            COLLATERAL_RELEASES[0],
        )
        .unwrap()
    }

    fn liabilities(authority: Id, outcomes: u8) -> MarketLiabilityFoundingPlanV3 {
        prepare_market_liability_founding_v3(
            collateral(),
            MarketLiabilityFoundingRequestV3 {
                hoard_account: id(6),
                claim_ledger_account: id(7),
                market_instance_id: id(3),
                native_claim_basis_id: id(8),
                claim_mint_authority: authority,
                outcome_count: outcomes,
                hoard_bump: 1,
                claim_ledger_bump: 2,
                hoard_rent: rent(50),
                claim_ledger_rent: rent(51),
            },
            &TestSha256,
        )
        .unwrap()
    }

    fn claim_mints(authority: Id, outcomes: u8, first_mint: Id) -> ClaimMintFoundingPlanV2 {
        let mut mints = [Id::ZERO; MAX_OUTCOMES];
        mints[0] = first_mint;
        if outcomes > 1 {
            mints[1] = id(21);
        }
        prepare_claim_mint_founding_v2(
            claim(),
            ClaimMintFoundingRequestV2 {
                market_instance_id: id(3),
                mint_authority: authority,
                outcome_count: outcomes,
                outcome_mints: mints,
            },
        )
        .unwrap()
    }

    fn outcome_custodies(
        authority: Id,
        outcomes: u8,
        first_mint: Id,
    ) -> OutcomeCustodyFoundingPlanV1 {
        let mut mints = [Id::ZERO; MAX_OUTCOMES];
        let mut custodies = [Id::ZERO; MAX_OUTCOMES];
        mints[0] = first_mint;
        custodies[0] = id(60);
        if outcomes > 1 {
            mints[1] = id(21);
            custodies[1] = id(61);
        }
        prepare_outcome_custody_founding_v1(
            collateral(),
            OutcomeCustodyFoundingRequestV1 {
                market_instance_id: id(3),
                generation: 1,
                owner_authority: authority,
                outcome_count: outcomes,
                outcome_mints: mints,
                outcome_custodies: custodies,
            },
        )
        .unwrap()
    }

    fn accepted_liabilities(
        bound: BoundCollateralProfileV2,
        plan: MarketLiabilityFoundingPlanV3,
    ) -> AcceptedMarketLiabilityFoundingV3 {
        let mut hoard_data = [0u8; HOARD_V2_BYTES];
        let mut claim_ledger_data = [0u8; CLAIM_LEDGER_V3_BYTES];
        plan.hoard().encode(&mut hoard_data).unwrap();
        plan.claim_ledger().encode(&mut claim_ledger_data).unwrap();
        let mut token_data = [0u8; 165];
        token_data[0..32].copy_from_slice(&id(22).bytes());
        token_data[32..64].copy_from_slice(&id(4).bytes());
        token_data[108] = 1;
        accept_market_liability_founding_v3(
            bound,
            plan,
            MarketLiabilityFoundingPostwriteV3 {
                hoard_account: id(6),
                hoard_data: &hoard_data,
                claim_ledger_account: id(7),
                claim_ledger_data: &claim_ledger_data,
                hoard_token: RuntimeAccountViewV2 {
                    key: id(5),
                    owner_program: LEGACY_SPL_TOKEN_PROGRAM,
                    data: &token_data,
                    is_signer: false,
                    is_writable: true,
                    executable: false,
                },
            },
        )
        .unwrap()
    }

    fn accepted_mint_step(
        claim: crate::BoundClaimIssuanceV1,
        plan: ClaimMintFoundingPlanV2,
        outcome: u8,
    ) -> AcceptedClaimMintFoundingStepV2 {
        let step = plan.step(outcome).unwrap();
        accept_claim_mint_founding_step_v2(
            claim,
            step,
            ClaimMintFoundingPostwriteV2 {
                mint: step.mint(),
                owner_program: TOKEN_2022_PROGRAM,
                writable: true,
                signer: false,
                executable: false,
                account_bytes: 82,
                initialized: true,
                decimals: 0,
                supply_atoms: 0,
                mint_authority: Some(step.mint_authority()),
                freeze_authority: None,
                extensions: 0,
            },
        )
        .unwrap()
    }

    fn accepted_custody_step(
        plan: OutcomeCustodyFoundingPlanV1,
        outcome: u8,
    ) -> AcceptedOutcomeCustodyFoundingStepV1 {
        let step = plan.step(outcome).unwrap();
        let mut token_data = [0u8; 165];
        token_data[0..32].copy_from_slice(&id(22).bytes());
        token_data[32..64].copy_from_slice(&step.owner_authority().bytes());
        token_data[108] = 1;
        accept_outcome_custody_founding_step_v1(
            step,
            RuntimeAccountViewV2 {
                key: step.outcome_custody(),
                owner_program: LEGACY_SPL_TOKEN_PROGRAM,
                data: &token_data,
                is_signer: false,
                is_writable: true,
                executable: false,
            },
        )
        .unwrap()
    }

    #[test]
    fn composes_exact_collateral_liability_and_claim_planes() {
        let bound = collateral();
        let plan = compose_market_core_founding_v4(
            bound,
            liabilities(id(11), 2),
            prepare_hoard_creation_v2(bound).unwrap(),
            claim_mints(id(11), 2, id(20)),
            outcome_custodies(id(11), 2, id(20)),
        )
        .unwrap();
        assert_eq!(plan.liabilities().claim_mint_authority(), id(11));
        assert_eq!(plan.claim_mints().mint_authority(), id(11));
        assert_eq!(plan.custody().account, id(5));
        assert!(!plan.core_id().is_zero());
    }

    #[test]
    fn refuses_cross_authority_width_or_collateral_mint_aliases() {
        let bound = collateral();
        let custody = prepare_hoard_creation_v2(bound).unwrap();
        assert_eq!(
            compose_market_core_founding_v4(
                bound,
                liabilities(id(11), 2),
                custody,
                claim_mints(id(12), 2, id(20)),
                outcome_custodies(id(11), 2, id(20)),
            ),
            Err(Error::MismatchedBinding)
        );
        assert_eq!(
            compose_market_core_founding_v4(
                bound,
                liabilities(id(11), 2),
                custody,
                claim_mints(id(11), 1, id(20)),
                outcome_custodies(id(11), 2, id(20)),
            ),
            Err(Error::MismatchedBinding)
        );
        assert_eq!(
            compose_market_core_founding_v4(
                bound,
                liabilities(id(11), 2),
                custody,
                claim_mints(id(11), 2, id(22)),
                outcome_custodies(id(11), 2, id(20)),
            ),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn core_acceptance_is_exhaustive_over_every_active_mint() {
        let bound = collateral();
        let liability_plan = liabilities(id(11), 2);
        let mint_plan = claim_mints(id(11), 2, id(20));
        let custody_plan = outcome_custodies(id(11), 2, id(20));
        let core = compose_market_core_founding_v4(
            bound,
            liability_plan,
            prepare_hoard_creation_v2(bound).unwrap(),
            mint_plan,
            custody_plan,
        )
        .unwrap();
        let liability_receipt = accepted_liabilities(bound, liability_plan);
        let claim = claim();
        let mut mints = [None; MAX_OUTCOMES];
        let mut custodies = [None; MAX_OUTCOMES];
        mints[0] = Some(accepted_mint_step(claim, mint_plan, 0));
        custodies[0] = Some(accepted_custody_step(custody_plan, 0));
        assert_eq!(
            accept_market_core_founding_v4(core, liability_receipt, mints, custodies),
            Err(Error::PostAdmissionFailed)
        );

        mints[1] = Some(accepted_mint_step(claim, mint_plan, 1));
        custodies[1] = Some(accepted_custody_step(custody_plan, 1));
        let accepted =
            accept_market_core_founding_v4(core, liability_receipt, mints, custodies).unwrap();
        assert_eq!(accepted.plan(), core);
        assert_eq!(
            accepted.liability_receipt_id(),
            liability_receipt.receipt_id()
        );
        assert!(!accepted.mint_receipt_ids()[0].is_zero());
        assert!(!accepted.mint_receipt_ids()[1].is_zero());
        assert_eq!(accepted.mint_receipt_ids()[2], Id::ZERO);
        assert!(!accepted.custody_receipt_ids()[0].is_zero());
        assert!(!accepted.custody_receipt_ids()[1].is_zero());
        assert_eq!(accepted.custody_receipt_ids()[2], Id::ZERO);
        assert!(!accepted.receipt_id().is_zero());
    }
}
