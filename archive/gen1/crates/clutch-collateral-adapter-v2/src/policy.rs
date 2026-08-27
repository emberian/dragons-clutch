// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer};
use crate::{digest, AdapterReleaseV2, Error, Id, Result};

const POLICY_MAGIC: [u8; 8] = *b"DCCPOL2\0";
const POLICY_VERSION: u16 = 2;
const POLICY_DOMAIN: &[u8] = b"dragons-clutch/collateral-policy/v2\0";
const POLICY_RESERVED_PREFIX_BYTES: usize = 7;
const POLICY_RESERVED_TAIL_BYTES: usize = 28;

/// Exact canonical V2 collateral policy width.
pub const COLLATERAL_POLICY_V2_BYTES: usize = 224;

/// Collateral mint must have no mint authority.
pub const POLICY_REQUIRE_MINT_AUTHORITY_NONE: u16 = 1 << 0;
/// Collateral mint must have no freeze authority.
pub const POLICY_REQUIRE_FREEZE_AUTHORITY_NONE: u16 = 1 << 1;
/// Current collateral mint supply must be nonzero.
pub const POLICY_REQUIRE_NONZERO_SUPPLY: u16 = 1 << 2;
/// Every custody account must have no delegate.
pub const POLICY_REQUIRE_CUSTODY_DELEGATE_NONE: u16 = 1 << 3;
/// Every custody account must have no close authority.
pub const POLICY_REQUIRE_CUSTODY_CLOSE_AUTHORITY_NONE: u16 = 1 << 4;
/// Native/wrapped-native token-account semantics are refused.
pub const POLICY_REQUIRE_NON_NATIVE_ACCOUNTS: u16 = 1 << 5;
/// Complete strict policy word admitted by V2.
pub const STRICT_POLICY_FLAGS: u16 = POLICY_REQUIRE_MINT_AUTHORITY_NONE
    | POLICY_REQUIRE_FREEZE_AUTHORITY_NONE
    | POLICY_REQUIRE_NONZERO_SUPPLY
    | POLICY_REQUIRE_CUSTODY_DELEGATE_NONE
    | POLICY_REQUIRE_CUSTODY_CLOSE_AUTHORITY_NONE
    | POLICY_REQUIRE_NON_NATIVE_ACCOUNTS;

/// Immutable Realm-selected collateral policy with an exact adapter release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralPolicyV2 {
    /// Strict authority and account-state policy word.
    pub flags: u16,
    /// Content identity of the exact compiled adapter release.
    pub adapter_release: Id,
    /// External token program address selected by that release.
    pub token_program: Id,
    /// Digest of the exact authenticated external program deployment/release.
    pub token_program_deployment: Id,
    /// Collateral mint address.
    pub mint: Id,
    /// Raw-atom exponent authenticated by checked transfers; never a rescale.
    pub decimals: u8,
    /// Maximum current collateral mint supply admitted by the Realm.
    pub max_supply_atoms: u64,
    /// Maximum cap any Market in this Realm may freeze in its Terms.
    pub max_market_collateral_atoms: u64,
    /// Mint extensions allowed by this Realm inside the release ceiling.
    pub allowed_mint_extensions: u64,
    /// Allowed mint extensions that must be present.
    pub required_mint_extensions: u64,
    /// Token-account extensions allowed inside the release ceiling.
    pub allowed_account_extensions: u64,
    /// Allowed extensions every collateral token account must carry.
    pub required_account_extensions: u64,
}

impl CollateralPolicyV2 {
    /// Construct a policy whose program/deployment fields come only from one
    /// already selected release rather than from parallel caller assertions.
    #[allow(clippy::too_many_arguments)]
    pub fn for_release(
        release: AdapterReleaseV2,
        mint: Id,
        decimals: u8,
        max_supply_atoms: u64,
        max_market_collateral_atoms: u64,
        allowed_mint_extensions: u64,
        required_mint_extensions: u64,
        allowed_account_extensions: u64,
        required_account_extensions: u64,
    ) -> Result<Self> {
        release.validate()?;
        let value = Self {
            flags: STRICT_POLICY_FLAGS,
            adapter_release: release.id()?,
            token_program: release.token_program,
            token_program_deployment: release.token_program_deployment,
            mint,
            decimals,
            max_supply_atoms,
            max_market_collateral_atoms,
            allowed_mint_extensions,
            required_mint_extensions,
            allowed_account_extensions,
            required_account_extensions,
        };
        value.validate_for_release(&release)?;
        Ok(value)
    }

    /// Validate canonical policy shape before resolving the release.
    pub fn validate(&self) -> Result<()> {
        self.adapter_release.require_live()?;
        self.token_program.require_live()?;
        self.token_program_deployment.require_live()?;
        self.mint.require_live()?;
        if self.flags != STRICT_POLICY_FLAGS
            || self.max_supply_atoms == 0
            || self.max_market_collateral_atoms == 0
            || self.max_market_collateral_atoms > self.max_supply_atoms
            || self.required_mint_extensions & !self.allowed_mint_extensions != 0
            || self.required_account_extensions & !self.allowed_account_extensions != 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Join this policy to one exact compiled release.
    pub fn validate_for_release(&self, release: &AdapterReleaseV2) -> Result<()> {
        self.validate()?;
        release.validate()?;
        if self.adapter_release != release.id()? {
            return Err(Error::UnknownAdapterRelease);
        }
        if self.token_program != release.token_program {
            return Err(Error::WrongProgram);
        }
        if self.token_program_deployment != release.token_program_deployment {
            return Err(Error::MismatchedBinding);
        }
        if self.allowed_mint_extensions & !release.safe_mint_extensions != 0
            || self.required_mint_extensions & !self.allowed_mint_extensions != 0
            || self.allowed_account_extensions & !release.safe_account_extensions != 0
            || self.required_account_extensions & !self.allowed_account_extensions != 0
            || release.required_custody_extensions & !self.allowed_account_extensions != 0
        {
            return Err(Error::ExtensionNotAdmitted);
        }
        Ok(())
    }

    /// Refuse a Market cap not frozen within the Realm policy ceiling.
    pub fn admit_market_cap(&self, market_cap_atoms: u64) -> Result<()> {
        self.validate()?;
        if market_cap_atoms == 0 || market_cap_atoms > self.max_market_collateral_atoms {
            Err(Error::MarketCapExceeded)
        } else {
            Ok(())
        }
    }

    /// Encode exact canonical policy bytes.
    pub fn encode(&self) -> Result<[u8; COLLATERAL_POLICY_V2_BYTES]> {
        self.validate()?;
        let mut output = [0; COLLATERAL_POLICY_V2_BYTES];
        let mut writer = Writer::new(&mut output, COLLATERAL_POLICY_V2_BYTES)?;
        writer.bytes(&POLICY_MAGIC)?;
        writer.u16(POLICY_VERSION)?;
        writer.u16(self.flags)?;
        writer.id(self.adapter_release)?;
        writer.id(self.token_program)?;
        writer.id(self.token_program_deployment)?;
        writer.id(self.mint)?;
        writer.u8(self.decimals)?;
        writer.bytes(&[0; POLICY_RESERVED_PREFIX_BYTES])?;
        writer.u64(self.max_supply_atoms)?;
        writer.u64(self.max_market_collateral_atoms)?;
        writer.u64(self.allowed_mint_extensions)?;
        writer.u64(self.required_mint_extensions)?;
        writer.u64(self.allowed_account_extensions)?;
        writer.u64(self.required_account_extensions)?;
        writer.bytes(&[0; POLICY_RESERVED_TAIL_BYTES])?;
        writer.finish()?;
        Ok(output)
    }

    /// Decode exact hostile bytes and refuse noncanonical tails or unknown V2s.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, COLLATERAL_POLICY_V2_BYTES)?;
        if reader.bytes::<8>()? != POLICY_MAGIC {
            return Err(Error::BadMagic);
        }
        if reader.u16()? != POLICY_VERSION {
            return Err(Error::BadVersion);
        }
        let value = Self {
            flags: reader.u16()?,
            adapter_release: reader.id()?,
            token_program: reader.id()?,
            token_program_deployment: reader.id()?,
            mint: reader.id()?,
            decimals: reader.u8()?,
            max_supply_atoms: {
                reader.require_zeroes(POLICY_RESERVED_PREFIX_BYTES)?;
                reader.u64()?
            },
            max_market_collateral_atoms: reader.u64()?,
            allowed_mint_extensions: reader.u64()?,
            required_mint_extensions: reader.u64()?,
            allowed_account_extensions: reader.u64()?,
            required_account_extensions: reader.u64()?,
        };
        reader.require_zeroes(POLICY_RESERVED_TAIL_BYTES)?;
        reader.finish()?;
        value.validate()?;
        if value.encode()?[..] != *input {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Domain-separated content identity committed by the parent Profile.
    pub fn id(&self) -> Result<Id> {
        let bytes = self.encode()?;
        Ok(digest(POLICY_DOMAIN, &[&bytes]))
    }
}
