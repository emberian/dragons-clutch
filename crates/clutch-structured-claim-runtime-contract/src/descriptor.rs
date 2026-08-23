//! Exact persisted descriptor and authenticated identity reconstruction.

use clutch_structured_claim::{
    BackingPlan, ClaimVector, DeploymentBinding, NativeBasisIdentity, NativeClaim,
    NATIVE_CLAIM_PREIMAGE_BYTES, WRAPPER_PRODUCT_PREIMAGE_BYTES,
};

use crate::{put, take, Error, Result, MAX_OUTCOMES};

/// Proposed structured-claim descriptor account discriminator.
///
/// The central collision registry must adopt this coordinate atomically with
/// the future SBF capability; this isolated pure contract does not allocate a
/// live account by itself.
pub const DESCRIPTOR_ACCOUNT_TAG: u8 = 0x88;
/// Structured-claim descriptor account version.
pub const DESCRIPTOR_ACCOUNT_VERSION: u8 = 1;
/// Exact descriptor account width frozen by the adapter plan.
pub const DESCRIPTOR_ACCOUNT_BYTES: usize = 384;

/// Descriptor lifecycle. Supply and backing remain outside this account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DescriptorStateV1 {
    /// Wrapper identity may execute supply-sensitive routes.
    Active = 0,
    /// Permanent zero-supply tombstone.
    Retired = 1,
}

impl DescriptorStateV1 {
    fn from_byte(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Retired),
            _ => Err(Error::InvalidState),
        }
    }
}

/// Exact 384-byte descriptor image.
///
/// The wrapper program is the account owner and PDA derivation program, so it
/// is intentionally reconstructed from authenticated account context rather
/// than persisted a second time. Outcome count, degree, and denominator are
/// likewise reconstructed from authenticated Market/Terms state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct StructuredClaimDescriptorV1 {
    /// Must equal [`DESCRIPTOR_ACCOUNT_TAG`].
    pub tag: u8,
    /// Must equal [`DESCRIPTOR_ACCOUNT_VERSION`].
    pub version: u8,
    /// Reserved flags; version one requires zero.
    pub flags: u16,
    /// Exact base Dragon's Clutch program.
    pub base_program: [u8; 32],
    /// Exact base ProgramData account.
    pub base_program_data: [u8; 32],
    /// Authenticated base deployment slot.
    pub base_deployment_slot: u64,
    /// Exact wrapper ProgramData account.
    pub wrapper_program_data: [u8; 32],
    /// Authenticated wrapper deployment slot.
    pub wrapper_deployment_slot: u64,
    /// Exact Token-2022 program.
    pub token_2022_program: [u8; 32],
    /// Exact Token-2022 ProgramData account.
    pub token_2022_program_data: [u8; 32],
    /// Authenticated Token-2022 deployment slot.
    pub token_2022_deployment_slot: u64,
    /// Canonical base Market account.
    pub market: [u8; 32],
    /// Complete immutable Terms digest.
    pub terms_digest: [u8; 32],
    /// Primitive GCD-one native-Egg coefficient vector.
    pub primitive: [u64; MAX_OUTCOMES],
    /// Active or permanently retired.
    pub state: DescriptorStateV1,
    /// Canonical descriptor PDA bump.
    pub descriptor_bump: u8,
    /// Canonical wrapper-mint PDA bump.
    pub mint_bump: u8,
    /// Canonical wrapper-vault-owner PDA bump.
    pub vault_bump: u8,
}

impl StructuredClaimDescriptorV1 {
    /// Encode the exact canonical account image.
    pub fn encode(&self) -> Result<[u8; DESCRIPTOR_ACCOUNT_BYTES]> {
        self.validate_persisted()?;
        let mut output = [0_u8; DESCRIPTOR_ACCOUNT_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &[self.tag])?;
        put(&mut output, &mut cursor, &[self.version])?;
        put(&mut output, &mut cursor, &self.flags.to_le_bytes())?;
        put(&mut output, &mut cursor, &self.base_program)?;
        put(&mut output, &mut cursor, &self.base_program_data)?;
        put(
            &mut output,
            &mut cursor,
            &self.base_deployment_slot.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &self.wrapper_program_data)?;
        put(
            &mut output,
            &mut cursor,
            &self.wrapper_deployment_slot.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &self.token_2022_program)?;
        put(&mut output, &mut cursor, &self.token_2022_program_data)?;
        put(
            &mut output,
            &mut cursor,
            &self.token_2022_deployment_slot.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &self.market)?;
        put(&mut output, &mut cursor, &self.terms_digest)?;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            put(
                &mut output,
                &mut cursor,
                &self.primitive[index].to_le_bytes(),
            )?;
            index += 1;
        }
        put(&mut output, &mut cursor, &[self.state as u8])?;
        put(&mut output, &mut cursor, &[self.descriptor_bump])?;
        put(&mut output, &mut cursor, &[self.mint_bump])?;
        put(&mut output, &mut cursor, &[self.vault_bump])?;
        if cursor != DESCRIPTOR_ACCOUNT_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Decode and validate an exact descriptor image.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != DESCRIPTOR_ACCOUNT_BYTES {
            return Err(Error::InvalidLength);
        }
        let mut cursor = 0_usize;
        let tag = take(input, &mut cursor, 1)?[0];
        let version = take(input, &mut cursor, 1)?[0];
        let flags = read_u16(input, &mut cursor)?;
        let base_program = read_key(input, &mut cursor)?;
        let base_program_data = read_key(input, &mut cursor)?;
        let base_deployment_slot = read_u64(input, &mut cursor)?;
        let wrapper_program_data = read_key(input, &mut cursor)?;
        let wrapper_deployment_slot = read_u64(input, &mut cursor)?;
        let token_2022_program = read_key(input, &mut cursor)?;
        let token_2022_program_data = read_key(input, &mut cursor)?;
        let token_2022_deployment_slot = read_u64(input, &mut cursor)?;
        let market = read_key(input, &mut cursor)?;
        let terms_digest = read_key(input, &mut cursor)?;
        let mut primitive = [0_u64; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            primitive[index] = read_u64(input, &mut cursor)?;
            index += 1;
        }
        let state = DescriptorStateV1::from_byte(take(input, &mut cursor, 1)?[0])?;
        let descriptor_bump = take(input, &mut cursor, 1)?[0];
        let mint_bump = take(input, &mut cursor, 1)?[0];
        let vault_bump = take(input, &mut cursor, 1)?[0];
        if cursor != input.len() {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            tag,
            version,
            flags,
            base_program,
            base_program_data,
            base_deployment_slot,
            wrapper_program_data,
            wrapper_deployment_slot,
            token_2022_program,
            token_2022_program_data,
            token_2022_deployment_slot,
            market,
            terms_digest,
            primitive,
            state,
            descriptor_bump,
            mint_bump,
            vault_bump,
        };
        value.validate_persisted()?;
        Ok(value)
    }

    /// Validate facts self-owned by the persisted image.
    pub fn validate_persisted(&self) -> Result<()> {
        if self.tag != DESCRIPTOR_ACCOUNT_TAG || self.version != DESCRIPTOR_ACCOUNT_VERSION {
            return Err(Error::InvalidHeader);
        }
        if self.flags != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        let keys = [
            self.base_program,
            self.base_program_data,
            self.wrapper_program_data,
            self.token_2022_program,
            self.token_2022_program_data,
            self.market,
        ];
        require_distinct_nonzero(&keys)?;
        if self.terms_digest == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

/// Basis facts reconstructed from authenticated Market and Terms accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DescriptorBasisV1 {
    /// Canonical Market identity.
    pub market: [u8; 32],
    /// Complete immutable Terms digest.
    pub terms_digest: [u8; 32],
    /// Frozen native basis degree.
    pub basis_degree: u8,
    /// Frozen exact simplex denominator.
    pub denominator: u64,
    /// Active outcome width.
    pub outcome_count: u8,
}

/// Reconstructed economic/deployment identity ready for adapter hashing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DescriptorIdentityV1 {
    /// Canonical native claim.
    pub claim: NativeClaim,
    /// Canonical complete-set-compressed backing.
    pub backing: BackingPlan,
    /// Authenticated executable deployment identities.
    pub deployment: DeploymentBinding,
    /// Exact bytes that the adapter must SHA-256 for the native claim id.
    pub native_claim_preimage: [u8; NATIVE_CLAIM_PREIMAGE_BYTES],
}

impl DescriptorIdentityV1 {
    /// Build the wrapper-product preimage from the adapter-computed native id.
    ///
    /// The caller must SHA-256 [`Self::native_claim_preimage`] and pass that
    /// exact digest; this pure crate intentionally contains no hash primitive.
    pub fn product_preimage(
        &self,
        native_claim_id: [u8; 32],
    ) -> Result<[u8; WRAPPER_PRODUCT_PREIMAGE_BYTES]> {
        self.deployment
            .product_preimage(native_claim_id)
            .map_err(|_| Error::InvalidIdentity)
    }
}

/// Join persisted descriptor bytes to authenticated basis and deployments.
pub fn reconstruct_descriptor_identity_v1(
    descriptor: &StructuredClaimDescriptorV1,
    basis: DescriptorBasisV1,
    deployment: DeploymentBinding,
) -> Result<DescriptorIdentityV1> {
    descriptor.validate_persisted()?;
    deployment.validate().map_err(|_| Error::InvalidIdentity)?;
    if descriptor.market != basis.market || descriptor.terms_digest != basis.terms_digest {
        return Err(Error::InvalidIdentity);
    }
    if descriptor.base_program != deployment.base_program
        || descriptor.base_program_data != deployment.base_program_data
        || descriptor.base_deployment_slot != deployment.base_deployment_slot
        || descriptor.wrapper_program_data != deployment.wrapper_program_data
        || descriptor.wrapper_deployment_slot != deployment.wrapper_deployment_slot
        || descriptor.token_2022_program != deployment.token_2022_program
        || descriptor.token_2022_program_data != deployment.token_2022_program_data
        || descriptor.token_2022_deployment_slot != deployment.token_2022_deployment_slot
    {
        return Err(Error::InvalidIdentity);
    }
    let claim = NativeClaim {
        basis: NativeBasisIdentity {
            market: basis.market,
            terms: basis.terms_digest,
            basis_degree: basis.basis_degree,
            denominator: basis.denominator,
            outcome_count: basis.outcome_count,
        },
        vector: ClaimVector {
            outcome_count: basis.outcome_count,
            coefficients: descriptor.primitive,
        },
    };
    claim.validate().map_err(|_| Error::InvalidClaim)?;
    let backing = claim
        .vector
        .backing_plan()
        .map_err(|_| Error::InvalidClaim)?;
    let native_claim_preimage = claim.identity_preimage().map_err(|_| Error::InvalidClaim)?;
    Ok(DescriptorIdentityV1 {
        claim,
        backing,
        deployment,
        native_claim_preimage,
    })
}

fn require_distinct_nonzero(keys: &[[u8; 32]]) -> Result<()> {
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

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0_u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut bytes = [0_u8; 2];
    bytes.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(bytes))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(bytes))
}
