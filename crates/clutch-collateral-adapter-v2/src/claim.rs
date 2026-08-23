// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer};
use crate::{digest, AdapterReleaseV2, Error, Id, Result, TOKEN_2022_PROGRAM};
use clutch_retirement::MAX_OUTCOMES;

const CLAIM_MAGIC: [u8; 8] = *b"DCCLAIM1";
const CLAIM_VERSION: u16 = 1;
const CLAIM_DOMAIN: &[u8] = b"dragons-clutch/claim-issuance-binding/v1\0";
const CLAIM_MINT_FOUNDING_DOMAIN_V2: &[u8] = b"dragons-clutch/claim-mint/founding/v2\0";
const CLAIM_MINT_FOUNDING_STEP_DOMAIN_V2: &[u8] = b"dragons-clutch/claim-mint/founding-step/v2\0";
const CLAIM_RESERVED_BYTES: usize = 3;

/// Exact canonical claim-issuance binding width.
pub const CLAIM_ISSUANCE_BINDING_V1_BYTES: usize = 160;
/// Exact extension-free Token-2022 mint width selected by claim release V1.
pub const CLAIM_MINT_ACCOUNT_BYTES_V2: usize = 82;
const CLAIM_MINT_ACCOUNT_BYTES_V2_WIRE: u16 = 82;

/// Claim release emits only protocol-owned mint/burn operations.
pub const CLAIM_FLAG_MINT_BURN_ONLY: u16 = 1 << 0;
/// Every claim mint has no freeze authority.
pub const CLAIM_FLAG_NO_FREEZE_AUTHORITY: u16 = 1 << 1;
/// Every claim mint uses raw indivisible atoms with decimals zero.
pub const CLAIM_FLAG_ZERO_DECIMALS: u16 = 1 << 2;
/// Claim issuance is fixed to the separately selected Token-2022 release.
pub const CLAIM_FLAG_TOKEN_2022: u16 = 1 << 3;
/// Complete V1 claim-plane flag word.
pub const CLAIM_FLAGS_V1: u16 = CLAIM_FLAG_MINT_BURN_ONLY
    | CLAIM_FLAG_NO_FREEZE_AUTHORITY
    | CLAIM_FLAG_ZERO_DECIMALS
    | CLAIM_FLAG_TOKEN_2022;

/// Independent identity of the Token-2022 Egg issuance plane.
///
/// This type is intentionally not embedded in [`crate::CollateralPolicyV2`]. A
/// legacy collateral Realm therefore still issues Token-2022 claims, and a
/// future claim release cannot mutate a Realm's collateral identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimIssuanceBindingV1 {
    /// Exact fixed semantic flag word.
    pub flags: u16,
    /// Content identity of the claim mint/burn adapter release.
    pub adapter_release: Id,
    /// Claim token program; V1 requires Token-2022.
    pub token_program: Id,
    /// Digest of the checked external Token-2022 deployment/release manifest.
    pub token_program_deployment: Id,
    /// Digest of the exact claim parser/CPI implementation in the Clutch build.
    pub parser_cpi_code: Id,
    /// Claim atom exponent; V1 requires zero.
    pub decimals: u8,
    /// Claim-mint extensions admitted by this issuance release; V1 requires none.
    pub mint_extensions: u64,
    /// Claim-account extensions imposed by issuance; V1 requires none.
    pub account_extensions: u64,
}

impl ClaimIssuanceBindingV1 {
    /// Validate the fixed V1 Token-2022 claim plane.
    pub fn validate(&self) -> Result<()> {
        self.adapter_release.require_live()?;
        self.token_program.require_live()?;
        self.token_program_deployment.require_live()?;
        self.parser_cpi_code.require_live()?;
        if self.flags != CLAIM_FLAGS_V1
            || self.token_program != TOKEN_2022_PROGRAM
            || self.decimals != 0
            || self.mint_extensions != 0
            || self.account_extensions != 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Encode exact canonical bytes.
    pub fn encode(&self) -> Result<[u8; CLAIM_ISSUANCE_BINDING_V1_BYTES]> {
        self.validate()?;
        let mut output = [0; CLAIM_ISSUANCE_BINDING_V1_BYTES];
        let mut writer = Writer::new(&mut output, CLAIM_ISSUANCE_BINDING_V1_BYTES)?;
        writer.bytes(&CLAIM_MAGIC)?;
        writer.u16(CLAIM_VERSION)?;
        writer.u16(self.flags)?;
        writer.id(self.adapter_release)?;
        writer.id(self.token_program)?;
        writer.id(self.token_program_deployment)?;
        writer.id(self.parser_cpi_code)?;
        writer.u8(self.decimals)?;
        writer.u64(self.mint_extensions)?;
        writer.u64(self.account_extensions)?;
        writer.bytes(&[0; CLAIM_RESERVED_BYTES])?;
        writer.finish()?;
        Ok(output)
    }

    /// Decode exact hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, CLAIM_ISSUANCE_BINDING_V1_BYTES)?;
        if reader.bytes::<8>()? != CLAIM_MAGIC {
            return Err(Error::BadMagic);
        }
        if reader.u16()? != CLAIM_VERSION {
            return Err(Error::BadVersion);
        }
        let value = Self {
            flags: reader.u16()?,
            adapter_release: reader.id()?,
            token_program: reader.id()?,
            token_program_deployment: reader.id()?,
            parser_cpi_code: reader.id()?,
            decimals: reader.u8()?,
            mint_extensions: reader.u64()?,
            account_extensions: reader.u64()?,
        };
        reader.require_zeroes(CLAIM_RESERVED_BYTES)?;
        reader.finish()?;
        value.validate()?;
        if value.encode()?[..] != *input {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Content identity referenced by an independently authenticated release manifest.
    pub fn id(&self) -> Result<Id> {
        let bytes = self.encode()?;
        Ok(digest(CLAIM_DOMAIN, &[&bytes]))
    }

    /// Refuse any accidental collapse of collateral and claim adapter releases.
    pub fn require_separate_from_collateral(
        &self,
        collateral_release: AdapterReleaseV2,
    ) -> Result<()> {
        self.validate()?;
        collateral_release.validate()?;
        if self.adapter_release == collateral_release.id()?
            || self.parser_cpi_code == collateral_release.parser_cpi_code
        {
            Err(Error::CollateralClaimPlaneAliased)
        } else {
            Ok(())
        }
    }
}

/// Runtime facts for the independent claim program/deployment boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimRuntimeObservationV1 {
    /// Presented Token-2022 program account.
    pub token_program: Id,
    /// Runtime executable bit.
    pub token_program_executable: bool,
    /// Runtime writable bit; must be false.
    pub token_program_writable: bool,
    /// Runtime signer bit; must be false.
    pub token_program_signer: bool,
    /// Digest recomputed from the authenticated external deployment manifest.
    pub token_program_deployment: Id,
    /// Digest of the executing claim parser/CPI component.
    pub parser_cpi_code: Id,
}

/// Fully joined claim plane, deliberately carrying no collateral transfer API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundClaimIssuanceV1 {
    binding_id: Id,
    binding: ClaimIssuanceBindingV1,
}

impl BoundClaimIssuanceV1 {
    /// Exact independently authenticated claim binding identity.
    pub const fn binding_id(self) -> Id {
        self.binding_id
    }

    /// Exact Token-2022 claim issuance binding.
    pub const fn binding(self) -> ClaimIssuanceBindingV1 {
        self.binding
    }
}

/// Join an independently expected claim binding to runtime deployment facts.
///
/// `expected_binding` must come from the checked release manifest or immutable
/// capability profile, never from collateral policy bytes.
pub fn bind_claim_issuance_v1(
    expected_binding: Id,
    binding: ClaimIssuanceBindingV1,
    runtime: ClaimRuntimeObservationV1,
    collateral_release: AdapterReleaseV2,
) -> Result<BoundClaimIssuanceV1> {
    expected_binding.require_live()?;
    runtime.token_program.require_live()?;
    runtime.token_program_deployment.require_live()?;
    runtime.parser_cpi_code.require_live()?;
    binding.validate()?;
    binding.require_separate_from_collateral(collateral_release)?;
    let binding_id = binding.id()?;
    if binding_id != expected_binding
        || runtime.token_program != binding.token_program
        || runtime.token_program_deployment != binding.token_program_deployment
        || runtime.parser_cpi_code != binding.parser_cpi_code
    {
        return Err(Error::MismatchedBinding);
    }
    if !runtime.token_program_executable
        || runtime.token_program_writable
        || runtime.token_program_signer
    {
        return Err(Error::WrongAccountRole);
    }
    Ok(BoundClaimIssuanceV1 {
        binding_id,
        binding,
    })
}

/// Exact fixed-width addresses for one full MarketInstanceV2 claim-mint plane.
///
/// This value is not authority. It is accepted only together with a
/// private-field [`BoundClaimIssuanceV1`] produced from the compiled claim
/// release and observed Token-2022 deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimMintFoundingRequestV2 {
    /// Full Product-owned MarketInstanceV2 identity.
    pub market_instance_id: Id,
    /// General MarketRuntime PDA that alone mints native claims.
    pub mint_authority: Id,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Canonical OutcomeMintV2 accounts in outcome order; inactive tail zero.
    pub outcome_mints: [Id; MAX_OUTCOMES],
}

/// Closed claim-mint creation contract for one full-width Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimMintFoundingPlanV2 {
    binding_id: Id,
    market_instance_id: Id,
    mint_authority: Id,
    outcome_count: u8,
    outcome_mints: [Id; MAX_OUTCOMES],
    founding_id: Id,
}

/// Exact one-mint projection from a complete Market claim-plane plan.
///
/// Product can use this bounded projection for one replay-counted founding
/// step without letting a caller substitute a mint or authority. A receipt
/// from this projection still does not activate the Market; Product owns the
/// exhaustive step counter and final activation transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimMintFoundingStepV2 {
    founding_id: Id,
    market_instance_id: Id,
    mint_authority: Id,
    outcome: u8,
    mint: Id,
    step_id: Id,
}

impl ClaimMintFoundingStepV2 {
    /// Complete claim-plane founding plan identity.
    pub const fn founding_id(self) -> Id {
        self.founding_id
    }

    /// Full MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> Id {
        self.market_instance_id
    }

    /// Sole General MarketRuntime mint authority.
    pub const fn mint_authority(self) -> Id {
        self.mint_authority
    }

    /// Exact active outcome index created by this step.
    pub const fn outcome(self) -> u8 {
        self.outcome
    }

    /// Exact canonical OutcomeMintV2 account created by this step.
    pub const fn mint(self) -> Id {
        self.mint
    }

    /// Replay-sensitive identity for this one bounded creation step.
    pub const fn step_id(self) -> Id {
        self.step_id
    }
}

impl ClaimMintFoundingPlanV2 {
    /// Exact independently authenticated claim release.
    pub const fn binding_id(self) -> Id {
        self.binding_id
    }

    /// Full MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> Id {
        self.market_instance_id
    }

    /// Sole General MarketRuntime mint authority.
    pub const fn mint_authority(self) -> Id {
        self.mint_authority
    }

    /// Active outcome width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Exact active mint, refusing inactive or out-of-range indices.
    pub fn outcome_mint(self, outcome: u8) -> Result<Id> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidParameter);
        }
        Ok(self.outcome_mints[usize::from(outcome)])
    }

    /// Canonical fixed-width mint vector with a zero inactive tail.
    pub const fn outcome_mints(self) -> [Id; MAX_OUTCOMES] {
        self.outcome_mints
    }

    /// Exact mint account width selected by claim release V1.
    pub const fn mint_account_bytes(self) -> usize {
        CLAIM_MINT_ACCOUNT_BYTES_V2
    }

    /// Shared claim-mint founding identity.
    pub const fn founding_id(self) -> Id {
        self.founding_id
    }

    /// Project one exact active mint for a bounded, replay-counted Product
    /// founding step.
    pub fn step(self, outcome: u8) -> Result<ClaimMintFoundingStepV2> {
        let mint = self.outcome_mint(outcome)?;
        let outcome_byte = [outcome];
        let step_id = digest(
            CLAIM_MINT_FOUNDING_STEP_DOMAIN_V2,
            &[
                &self.founding_id.bytes(),
                &self.market_instance_id.bytes(),
                &self.mint_authority.bytes(),
                &outcome_byte,
                &mint.bytes(),
            ],
        );
        step_id.require_live()?;
        Ok(ClaimMintFoundingStepV2 {
            founding_id: self.founding_id,
            market_instance_id: self.market_instance_id,
            mint_authority: self.mint_authority,
            outcome,
            mint,
            step_id,
        })
    }
}

/// Bind all canonical OutcomeMintV2 addresses to the independent claim
/// release and one exact General MarketRuntime authority.
///
/// PDA derivation and absence remain runtime obligations. This constructor
/// prevents a writer from mixing releases, authorities, Markets, duplicate
/// mints, or a noncanonical inactive tail.
pub fn prepare_claim_mint_founding_v2(
    claim: BoundClaimIssuanceV1,
    request: ClaimMintFoundingRequestV2,
) -> Result<ClaimMintFoundingPlanV2> {
    request.market_instance_id.require_live()?;
    request.mint_authority.require_live()?;
    if request.outcome_count == 0 || usize::from(request.outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidParameter);
    }
    let binding = claim.binding();
    binding.validate()?;
    if binding.decimals != 0
        || binding.mint_extensions != 0
        || binding.account_extensions != 0
        || binding.token_program != TOKEN_2022_PROGRAM
    {
        return Err(Error::MismatchedBinding);
    }

    let active = usize::from(request.outcome_count);
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        let mint = request.outcome_mints[index];
        if index < active {
            mint.require_live()?;
            if mint == request.market_instance_id || mint == request.mint_authority {
                return Err(Error::MismatchedBinding);
            }
            let mut prior = 0usize;
            while prior < index {
                if request.outcome_mints[prior] == mint {
                    return Err(Error::MismatchedBinding);
                }
                prior += 1;
            }
        } else if !mint.is_zero() {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }

    let mut outcome_count = [0u8; 1];
    outcome_count[0] = request.outcome_count;
    let mut mint_bytes = [[0u8; 32]; MAX_OUTCOMES];
    index = 0;
    while index < MAX_OUTCOMES {
        mint_bytes[index] = request.outcome_mints[index].bytes();
        index += 1;
    }
    let mut parts: [&[u8]; 5 + MAX_OUTCOMES] = [&[]; 5 + MAX_OUTCOMES];
    let binding_id = claim.binding_id();
    let binding_bytes = binding_id.bytes();
    let market_bytes = request.market_instance_id.bytes();
    let authority_bytes = request.mint_authority.bytes();
    let account_bytes = CLAIM_MINT_ACCOUNT_BYTES_V2_WIRE.to_le_bytes();
    parts[0] = &binding_bytes;
    parts[1] = &market_bytes;
    parts[2] = &authority_bytes;
    parts[3] = &outcome_count;
    parts[4] = &account_bytes;
    index = 0;
    while index < MAX_OUTCOMES {
        parts[5 + index] = &mint_bytes[index];
        index += 1;
    }
    let founding_id = digest(CLAIM_MINT_FOUNDING_DOMAIN_V2, &parts);
    founding_id.require_live()?;
    Ok(ClaimMintFoundingPlanV2 {
        binding_id,
        market_instance_id: request.market_instance_id,
        mint_authority: request.mint_authority,
        outcome_count: request.outcome_count,
        outcome_mints: request.outcome_mints,
        founding_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn bound_claim() -> BoundClaimIssuanceV1 {
        let collateral_release = AdapterReleaseV2::legacy_spl(id(1), id(2));
        let binding = ClaimIssuanceBindingV1 {
            flags: CLAIM_FLAGS_V1,
            adapter_release: id(3),
            token_program: TOKEN_2022_PROGRAM,
            token_program_deployment: id(4),
            parser_cpi_code: id(5),
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
                token_program_deployment: id(4),
                parser_cpi_code: id(5),
            },
            collateral_release,
        )
        .unwrap()
    }

    fn request() -> ClaimMintFoundingRequestV2 {
        let mut outcome_mints = [Id::ZERO; MAX_OUTCOMES];
        outcome_mints[0] = id(20);
        outcome_mints[1] = id(21);
        ClaimMintFoundingRequestV2 {
            market_instance_id: id(10),
            mint_authority: id(11),
            outcome_count: 2,
            outcome_mints,
        }
    }

    #[test]
    fn founding_plan_commits_ordered_mints_and_bounded_steps() {
        let plan = prepare_claim_mint_founding_v2(bound_claim(), request()).unwrap();
        assert_eq!(plan.outcome_mint(0), Ok(id(20)));
        assert_eq!(plan.outcome_mint(1), Ok(id(21)));
        assert_eq!(plan.outcome_mint(2), Err(Error::InvalidParameter));
        assert_eq!(plan.mint_account_bytes(), CLAIM_MINT_ACCOUNT_BYTES_V2);
        let first = plan.step(0).unwrap();
        let second = plan.step(1).unwrap();
        assert_eq!(first.mint(), id(20));
        assert_eq!(first.mint_authority(), id(11));
        assert_eq!(first.founding_id(), plan.founding_id());
        assert_ne!(first.step_id(), second.step_id());
    }

    #[test]
    fn founding_plan_refuses_duplicate_mints_and_nonzero_tail() {
        let mut duplicate = request();
        duplicate.outcome_mints[1] = duplicate.outcome_mints[0];
        assert_eq!(
            prepare_claim_mint_founding_v2(bound_claim(), duplicate),
            Err(Error::MismatchedBinding)
        );

        let mut dirty_tail = request();
        dirty_tail.outcome_mints[2] = id(22);
        assert_eq!(
            prepare_claim_mint_founding_v2(bound_claim(), dirty_tail),
            Err(Error::NonCanonicalPadding)
        );
    }
}
