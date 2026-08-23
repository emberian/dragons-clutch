// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer};
use crate::{digest, AdapterReleaseV2, Error, Id, Result, TOKEN_2022_PROGRAM};

const CLAIM_MAGIC: [u8; 8] = *b"DCCLAIM1";
const CLAIM_VERSION: u16 = 1;
const CLAIM_DOMAIN: &[u8] = b"dragons-clutch/claim-issuance-binding/v1\0";
const CLAIM_RESERVED_BYTES: usize = 3;

/// Exact canonical claim-issuance binding width.
pub const CLAIM_ISSUANCE_BINDING_V1_BYTES: usize = 160;

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
