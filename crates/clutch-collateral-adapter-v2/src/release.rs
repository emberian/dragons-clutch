// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer};
use crate::{digest, Error, Id, Result};

/// Canonical legacy SPL Token program address.
pub const LEGACY_SPL_TOKEN_PROGRAM: Id = Id::from_bytes([
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
]);

/// Canonical Token-2022 program address.
pub const TOKEN_2022_PROGRAM: Id = Id::from_bytes([
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
]);

/// Base mint layout shared by legacy SPL Token and Token-2022.
pub const BASE_MINT_BYTES: u16 = 82;
/// Base token-account layout shared by legacy SPL Token and Token-2022.
pub const BASE_TOKEN_ACCOUNT_BYTES: u16 = 165;
/// Token-2022 account with only the zero-width `ImmutableOwner` extension.
pub const IMMUTABLE_OWNER_ACCOUNT_BYTES: u16 = 170;
/// Token-2022 extension discriminants `0..=28` pinned by this parser release.
pub const TOKEN_2022_KNOWN_EXTENSIONS: u64 = (1_u64 << 29) - 1;
/// Token-2022 `ImmutableOwner` extension bit.
pub const EXTENSION_IMMUTABLE_OWNER: u64 = 1_u64 << 7;

const RELEASE_MAGIC: [u8; 8] = *b"DCCAR2\0\0";
const RELEASE_VERSION: u16 = 2;
const RELEASE_DOMAIN: &[u8] = b"dragons-clutch/collateral-adapter-release/v2\0";
const RELEASE_RESERVED_BYTES: usize = 30;

/// Exact canonical release-record width.
pub const ADAPTER_RELEASE_V2_BYTES: usize = 192;
/// Maximum entries in one compile-time closed collateral release catalog.
pub const MAX_ADAPTER_RELEASES: usize = 16;

/// Release supports Hoard creation and initialization.
pub const OPERATION_CREATE_HOARD: u16 = 1 << 0;
/// Release supports exact checked transfers from holders into custody.
pub const OPERATION_TRANSFER_IN: u16 = 1 << 1;
/// Release supports exact checked transfers from custody to holders.
pub const OPERATION_TRANSFER_OUT: u16 = 1 << 2;
/// Release supports exact checked transfers between authenticated custody roles.
pub const OPERATION_CUSTODY_TRANSFER: u16 = 1 << 3;
/// Complete operation surface required by V2.
pub const REQUIRED_OPERATIONS: u16 = OPERATION_CREATE_HOARD
    | OPERATION_TRANSFER_IN
    | OPERATION_TRANSFER_OUT
    | OPERATION_CUSTODY_TRANSFER;

/// Release proves visible integer amounts and mint supply.
pub const RELEASE_FLAG_VISIBLE_INTEGER_ATOMS: u16 = 1 << 0;
/// Release proves exact one-to-one transfer semantics before live postchecks.
pub const RELEASE_FLAG_EXACT_ONE_TO_ONE: u16 = 1 << 1;
/// Release/parser combination excludes unenumerated foreign invocations.
pub const RELEASE_FLAG_NO_FOREIGN_INVOCATION: u16 = 1 << 2;
/// Clutch's collateral surface exposes no token-account owner change.
pub const RELEASE_FLAG_NO_OWNER_AUTHORITY_CHANGE: u16 = 1 << 3;
/// Complete exact-visible-atom release guarantees required by V2.
pub const REQUIRED_RELEASE_FLAGS: u16 = RELEASE_FLAG_VISIBLE_INTEGER_ATOMS
    | RELEASE_FLAG_EXACT_ONE_TO_ONE
    | RELEASE_FLAG_NO_FOREIGN_INVOCATION
    | RELEASE_FLAG_NO_OWNER_AUTHORITY_CHANGE;

/// Dangerous semantic behavior flags. V2 exact-atom releases require zero.
pub mod behavior {
    /// Transfer debit and destination credit may differ due to a fee.
    pub const FEE_ON_TRANSFER: u16 = 1 << 0;
    /// Transfer can invoke a caller-selected or mutable foreign hook.
    pub const TRANSFER_HOOK: u16 = 1 << 1;
    /// Amounts, supply, or auxiliary balances are not exact visible integers.
    pub const CONFIDENTIAL: u16 = 1 << 2;
    /// Ordinary transfer may be disabled by nontransferable state.
    pub const NONTRANSFERABLE: u16 = 1 << 3;
    /// Accounts may default frozen under mutable external policy.
    pub const DEFAULT_FROZEN: u16 = 1 << 4;
    /// A permanent third-party delegate can seize or burn collateral.
    pub const PERMANENT_DELEGATE: u16 = 1 << 5;
    /// An external authority can pause ordinary transfers.
    pub const PAUSABLE: u16 = 1 << 6;
    /// Raw atoms have mutable display or time-dependent scaling semantics.
    pub const SCALED_UNITS: u16 = 1 << 7;
    /// Every dangerous behavior understood by release schema V2.
    pub const ALL: u16 = FEE_ON_TRANSFER
        | TRANSFER_HOOK
        | CONFIDENTIAL
        | NONTRANSFERABLE
        | DEFAULT_FROZEN
        | PERMANENT_DELEGATE
        | PAUSABLE
        | SCALED_UNITS;
}

/// Concrete parser/CPI implementation family compiled into the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ProgramFamilyV2 {
    /// Fixed 82/165-byte legacy SPL Token layouts.
    LegacySpl = 1,
    /// Token-2022 under the pinned 29-discriminant parser ceiling.
    Token2022Base = 2,
}

impl ProgramFamilyV2 {
    fn decode(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::LegacySpl),
            2 => Ok(Self::Token2022Base),
            _ => Err(Error::BadVersion),
        }
    }
}

/// How custody prevents a caller from changing its token owner authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OwnerGuardV2 {
    /// Token-2022 enforces `ImmutableOwner` in the account state.
    ImmutableOwner = 1,
    /// The owner is a canonical program-derived address and this release has no
    /// owner-authority-change route.
    PdaSoleSigner = 2,
}

impl OwnerGuardV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::ImmutableOwner),
            2 => Ok(Self::PdaSoleSigner),
            _ => Err(Error::BadVersion),
        }
    }
}

/// Canonical semantic contract of one compiled collateral adapter release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterReleaseV2 {
    /// Family-specific hostile-byte parser and CPI constructor.
    pub family: ProgramFamilyV2,
    /// Custody owner guard furnished by the family.
    pub owner_guard: OwnerGuardV2,
    /// Checked-transfer instruction discriminator emitted by this release.
    pub transfer_checked_discriminator: u8,
    /// Exact supported operation mask.
    pub supported_operations: u16,
    /// Exact semantic guarantee flags.
    pub release_flags: u16,
    /// Dangerous behavior intrinsic to the release; V2 requires zero.
    pub intrinsic_behaviors: u16,
    /// External token program address.
    pub token_program: Id,
    /// Digest of the authenticated external deployment/release manifest.
    pub token_program_deployment: Id,
    /// Digest of the exact parser and CPI implementation in the Clutch build.
    pub parser_cpi_code: Id,
    /// Extension discriminants the mint parser recognizes and refuses safely.
    pub known_mint_extensions: u64,
    /// Recognized mint extensions compatible with exact visible atoms.
    pub safe_mint_extensions: u64,
    /// Extension discriminants the account parser recognizes and refuses safely.
    pub known_account_extensions: u64,
    /// Recognized account extensions compatible with exact visible atoms.
    pub safe_account_extensions: u64,
    /// Extensions mandatory on every custody account under this release.
    pub required_custody_extensions: u64,
    /// Exact collateral mint-account byte length.
    pub mint_account_bytes: u16,
    /// Exact extension-free holder account byte length.
    pub holder_account_bytes: u16,
    /// Exact custody token-account byte length.
    pub custody_account_bytes: u16,
}

impl AdapterReleaseV2 {
    /// Build the exact legacy SPL family row around real deployment and
    /// parser/CPI component identities. Validation still rejects zero ids.
    pub const fn legacy_spl(
        token_program_deployment: Id,
        parser_cpi_code: Id,
    ) -> Self {
        Self {
            family: ProgramFamilyV2::LegacySpl,
            owner_guard: OwnerGuardV2::PdaSoleSigner,
            transfer_checked_discriminator: 12,
            supported_operations: REQUIRED_OPERATIONS,
            release_flags: REQUIRED_RELEASE_FLAGS,
            intrinsic_behaviors: 0,
            token_program: LEGACY_SPL_TOKEN_PROGRAM,
            token_program_deployment,
            parser_cpi_code,
            known_mint_extensions: 0,
            safe_mint_extensions: 0,
            known_account_extensions: 0,
            safe_account_extensions: 0,
            required_custody_extensions: 0,
            mint_account_bytes: BASE_MINT_BYTES,
            holder_account_bytes: BASE_TOKEN_ACCOUNT_BYTES,
            custody_account_bytes: BASE_TOKEN_ACCOUNT_BYTES,
        }
    }

    /// Build the exact conservative Token-2022 family row around real
    /// deployment and parser/CPI component identities.
    pub const fn token_2022_base(
        token_program_deployment: Id,
        parser_cpi_code: Id,
    ) -> Self {
        Self {
            family: ProgramFamilyV2::Token2022Base,
            owner_guard: OwnerGuardV2::ImmutableOwner,
            transfer_checked_discriminator: 12,
            supported_operations: REQUIRED_OPERATIONS,
            release_flags: REQUIRED_RELEASE_FLAGS,
            intrinsic_behaviors: 0,
            token_program: TOKEN_2022_PROGRAM,
            token_program_deployment,
            parser_cpi_code,
            known_mint_extensions: TOKEN_2022_KNOWN_EXTENSIONS,
            safe_mint_extensions: 0,
            known_account_extensions: TOKEN_2022_KNOWN_EXTENSIONS,
            safe_account_extensions: EXTENSION_IMMUTABLE_OWNER,
            required_custody_extensions: EXTENSION_IMMUTABLE_OWNER,
            mint_account_bytes: BASE_MINT_BYTES,
            holder_account_bytes: BASE_TOKEN_ACCOUNT_BYTES,
            custody_account_bytes: IMMUTABLE_OWNER_ACCOUNT_BYTES,
        }
    }

    /// Validate a release independently of any Realm policy.
    pub fn validate(&self) -> Result<()> {
        self.token_program.require_live()?;
        self.token_program_deployment.require_live()?;
        self.parser_cpi_code.require_live()?;
        if self.transfer_checked_discriminator != 12
            || self.supported_operations != REQUIRED_OPERATIONS
            || self.release_flags != REQUIRED_RELEASE_FLAGS
            || self.intrinsic_behaviors & behavior::ALL != 0
            || self.safe_mint_extensions & !self.known_mint_extensions != 0
            || self.safe_account_extensions & !self.known_account_extensions != 0
            || self.required_custody_extensions & !self.safe_account_extensions != 0
        {
            return Err(Error::InvalidParameter);
        }
        match self.family {
            ProgramFamilyV2::LegacySpl => {
                if self.token_program != LEGACY_SPL_TOKEN_PROGRAM
                    || self.known_mint_extensions != 0
                    || self.safe_mint_extensions != 0
                    || self.known_account_extensions != 0
                    || self.safe_account_extensions != 0
                    || self.required_custody_extensions != 0
                    || self.owner_guard != OwnerGuardV2::PdaSoleSigner
                    || self.mint_account_bytes != BASE_MINT_BYTES
                    || self.holder_account_bytes != BASE_TOKEN_ACCOUNT_BYTES
                    || self.custody_account_bytes != BASE_TOKEN_ACCOUNT_BYTES
                {
                    return Err(Error::InvalidParameter);
                }
            }
            ProgramFamilyV2::Token2022Base => {
                if self.token_program != TOKEN_2022_PROGRAM
                    || self.known_mint_extensions != TOKEN_2022_KNOWN_EXTENSIONS
                    || self.safe_mint_extensions != 0
                    || self.known_account_extensions != TOKEN_2022_KNOWN_EXTENSIONS
                    || self.safe_account_extensions != EXTENSION_IMMUTABLE_OWNER
                    || self.required_custody_extensions != EXTENSION_IMMUTABLE_OWNER
                    || self.owner_guard != OwnerGuardV2::ImmutableOwner
                    || self.mint_account_bytes != BASE_MINT_BYTES
                    || self.holder_account_bytes != BASE_TOKEN_ACCOUNT_BYTES
                    || self.custody_account_bytes != IMMUTABLE_OWNER_ACCOUNT_BYTES
                {
                    return Err(Error::InvalidParameter);
                }
            }
        }
        Ok(())
    }

    /// Encode the exact canonical release record.
    pub fn encode(&self) -> Result<[u8; ADAPTER_RELEASE_V2_BYTES]> {
        self.validate()?;
        let mut output = [0; ADAPTER_RELEASE_V2_BYTES];
        let mut writer = Writer::new(&mut output, ADAPTER_RELEASE_V2_BYTES)?;
        writer.bytes(&RELEASE_MAGIC)?;
        writer.u16(RELEASE_VERSION)?;
        writer.u16(self.family as u16)?;
        writer.u8(self.owner_guard as u8)?;
        writer.u8(self.transfer_checked_discriminator)?;
        writer.u16(self.supported_operations)?;
        writer.u16(self.release_flags)?;
        writer.u16(self.intrinsic_behaviors)?;
        writer.id(self.token_program)?;
        writer.id(self.token_program_deployment)?;
        writer.id(self.parser_cpi_code)?;
        writer.u64(self.known_mint_extensions)?;
        writer.u64(self.safe_mint_extensions)?;
        writer.u64(self.known_account_extensions)?;
        writer.u64(self.safe_account_extensions)?;
        writer.u64(self.required_custody_extensions)?;
        writer.u16(self.mint_account_bytes)?;
        writer.u16(self.holder_account_bytes)?;
        writer.u16(self.custody_account_bytes)?;
        writer.bytes(&[0; RELEASE_RESERVED_BYTES])?;
        writer.finish()?;
        Ok(output)
    }

    /// Decode exact hostile bytes and refuse unknown versions or families.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, ADAPTER_RELEASE_V2_BYTES)?;
        if reader.bytes::<8>()? != RELEASE_MAGIC {
            return Err(Error::BadMagic);
        }
        if reader.u16()? != RELEASE_VERSION {
            return Err(Error::BadVersion);
        }
        let value = Self {
            family: ProgramFamilyV2::decode(reader.u16()?)?,
            owner_guard: OwnerGuardV2::decode(reader.u8()?)?,
            transfer_checked_discriminator: reader.u8()?,
            supported_operations: reader.u16()?,
            release_flags: reader.u16()?,
            intrinsic_behaviors: reader.u16()?,
            token_program: reader.id()?,
            token_program_deployment: reader.id()?,
            parser_cpi_code: reader.id()?,
            known_mint_extensions: reader.u64()?,
            safe_mint_extensions: reader.u64()?,
            known_account_extensions: reader.u64()?,
            safe_account_extensions: reader.u64()?,
            required_custody_extensions: reader.u64()?,
            mint_account_bytes: reader.u16()?,
            holder_account_bytes: reader.u16()?,
            custody_account_bytes: reader.u16()?,
        };
        reader.require_zeroes(RELEASE_RESERVED_BYTES)?;
        reader.finish()?;
        value.validate()?;
        if value.encode()?[..] != *input {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Content identity selected by a V2 Realm collateral policy.
    pub fn id(&self) -> Result<Id> {
        let bytes = self.encode()?;
        Ok(digest(RELEASE_DOMAIN, &[&bytes]))
    }
}

/// Bounded compile-time closed catalog. There is intentionally no default row:
/// a deployable crate must supply real code and deployment identities.
#[derive(Clone, Copy, Debug)]
pub struct AdapterCatalogV2 {
    releases: &'static [AdapterReleaseV2],
}

impl AdapterCatalogV2 {
    /// Construct a bounded catalog and refuse duplicate release identities.
    pub fn new(releases: &'static [AdapterReleaseV2]) -> Result<Self> {
        if releases.is_empty() || releases.len() > MAX_ADAPTER_RELEASES {
            return Err(Error::InvalidParameter);
        }
        let mut left = 0;
        while left < releases.len() {
            releases[left].validate()?;
            let left_id = releases[left].id()?;
            let mut right = left + 1;
            while right < releases.len() {
                if left_id == releases[right].id()? {
                    return Err(Error::DuplicateAdapterRelease);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(Self { releases })
    }

    /// Resolve only an exact release identity present in the compiled catalog.
    pub fn resolve(&self, selected: Id) -> Result<AdapterReleaseV2> {
        selected.require_live()?;
        let mut index = 0;
        while index < self.releases.len() {
            if self.releases[index].id()? == selected {
                return Ok(self.releases[index]);
            }
            index += 1;
        }
        Err(Error::UnknownAdapterRelease)
    }

    /// Number of compiled release rows.
    pub const fn len(&self) -> usize {
        self.releases.len()
    }

    /// Whether the catalog has no rows. Construction refuses this state.
    pub const fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }
}
