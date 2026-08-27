//! Deployment, digest, and program-derived-address authentication.

use crate::runtime_contract::{
    reconstruct_descriptor_identity_v1, DescriptorBasisV1, DescriptorIdentityV1,
    StructuredClaimDescriptorV2, StructuredClaimRuntimeAddressesV1, WrapperRecipeV1,
    DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION, WRAPPER_RECIPE_ID_DOMAIN_V1,
};
use clutch_structured_claim::DeploymentBinding;
#[cfg(not(target_os = "solana"))]
use sha2::{Digest, Sha256};

use crate::{is_zero, Error, Key, Result};

/// Descriptor PDA seed prefix.
pub const DESCRIPTOR_SEED: &[u8] = b"dc:claim-desc:v1";
/// Extension-free wrapper-mint PDA seed prefix.
pub const MINT_SEED: &[u8] = b"dc:claim-mint:v1";
/// Token-2022 mint-authority PDA seed prefix.
pub const MINT_AUTHORITY_SEED: &[u8] = b"dc:claim-mint-auth:v1";
/// Base Position semantic-owner PDA seed prefix.
pub const VAULT_OWNER_SEED: &[u8] = b"dc:claim-vault:v1";
/// Series-link-scoped wrapper-product identity domain.
pub const SERIES_SCOPED_WRAPPER_PRODUCT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/structured-claim/series-scoped-wrapper-product/v2\0";

const _: () = assert!(DESCRIPTOR_ACCOUNT_TAG == 0x88);
const _: () = assert!(DESCRIPTOR_ACCOUNT_VERSION == 2);

/// Authenticated executable and ProgramData observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeDeploymentsV1 {
    /// Canonical runtime-contract deployment identity.
    pub binding: DeploymentBinding,
    /// Pinned upgradeable-loader executable.
    pub upgradeable_loader: Key,
    /// Owners of wrapper, base, and Token-2022 Program accounts.
    pub program_owners: [Key; 3],
    /// Owners of wrapper, base, and Token-2022 ProgramData accounts.
    pub program_data_owners: [Key; 3],
    /// ProgramData addresses linked by those Program accounts.
    pub linked_program_data: [Key; 3],
    /// Executable bits for wrapper, base, and Token-2022; exactly `0b111`.
    pub executable_mask: u8,
}

impl RuntimeDeploymentsV1 {
    /// Validate exact loader ownership, linkage, and executable identity.
    pub fn validate(&self) -> Result<()> {
        self.binding
            .validate()
            .map_err(|_| Error::InvalidDeployment)?;
        if is_zero(&self.upgradeable_loader) || self.executable_mask != 0b111 {
            return Err(Error::InvalidDeployment);
        }
        let expected = [
            self.binding.wrapper_program_data,
            self.binding.base_program_data,
            self.binding.token_2022_program_data,
        ];
        let mut index = 0_usize;
        while index < expected.len() {
            if self.program_owners[index] != self.upgradeable_loader
                || self.program_data_owners[index] != self.upgradeable_loader
                || self.linked_program_data[index] != expected[index]
            {
                return Err(Error::InvalidDeployment);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Program-address verifier supplied by the target-specific adapter boundary.
pub trait PdaVerifierV1 {
    /// Return true only for the exact PDA at `(program, prefix, product, bump)`.
    fn verify(
        &self,
        program: &Key,
        address: &Key,
        prefix: &[u8],
        product_id: &Key,
        bump: u8,
    ) -> bool;
}

/// Solana syscall-backed program-address verifier.
#[cfg(target_os = "solana")]
#[derive(Clone, Copy, Debug, Default)]
pub struct SolanaPdaVerifierV1;

#[cfg(target_os = "solana")]
impl PdaVerifierV1 for SolanaPdaVerifierV1 {
    fn verify(
        &self,
        program: &Key,
        address: &Key,
        prefix: &[u8],
        product_id: &Key,
        bump: u8,
    ) -> bool {
        use solana_pubkey::Pubkey;

        let bump_seed = [bump];
        let program = Pubkey::new_from_array(*program);
        Pubkey::create_program_address(&[prefix, product_id, &bump_seed], &program)
            .map(|derived| derived.to_bytes() == *address)
            .unwrap_or(false)
    }
}

/// Fully bound canonical descriptor identity.
///
/// Fields are private so downstream code cannot detach a caller-authored
/// identity from the deployment/hash/PDA checks that minted this value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundDescriptorV1 {
    descriptor: StructuredClaimDescriptorV2,
    identity: DescriptorIdentityV1,
    native_claim_id: Key,
    wrapper_product_id: Key,
    addresses: StructuredClaimRuntimeAddressesV1,
}

impl BoundDescriptorV1 {
    /// Canonical persisted descriptor.
    pub const fn descriptor(&self) -> &StructuredClaimDescriptorV2 {
        &self.descriptor
    }

    /// Runtime-contract identity reconstructed from authenticated basis.
    pub const fn identity(&self) -> &DescriptorIdentityV1 {
        &self.identity
    }

    /// Canonical native-claim digest.
    pub const fn native_claim_id(&self) -> Key {
        self.native_claim_id
    }

    /// Canonical deployment-bound wrapper-product digest.
    pub const fn wrapper_product_id(&self) -> Key {
        self.wrapper_product_id
    }

    /// Canonical descriptor, mint, authority, and vault-owner addresses.
    pub const fn addresses(&self) -> StructuredClaimRuntimeAddressesV1 {
        self.addresses
    }
}

/// SHA-256 the exact runtime-contract native-claim preimage.
pub fn canonical_native_claim_id_v1(identity: &DescriptorIdentityV1) -> Result<Key> {
    hash(&identity.native_claim_preimage)
}

/// SHA-256 the historical deployment-only wrapper-product component.
///
/// This helper is intentionally private: only the current Series/root/recipe-
/// scoped identity may cross the adapter API as an executable product.
fn canonical_wrapper_product_id_v1(
    identity: &DescriptorIdentityV1,
    native_claim_id: Key,
) -> Result<Key> {
    hash(&identity.product_preimage(native_claim_id)?)
}

/// Bind the deployment/native product to one exact Structured root and recipe.
pub fn canonical_series_scoped_wrapper_product_id_v2(
    identity: &DescriptorIdentityV1,
    native_claim_id: Key,
    structured_root_id: Key,
    wrapper_recipe_id: Key,
) -> Result<Key> {
    if is_zero(&structured_root_id)
        || is_zero(&wrapper_recipe_id)
        || structured_root_id == wrapper_recipe_id
    {
        return Err(Error::DigestMismatch);
    }
    let recipe_preimage = WrapperRecipeV1 {
        native_claim_id,
        outcome_count: identity.claim.basis.outcome_count,
        primitive: identity.claim.vector.coefficients,
    }
    .encode_preimage()
    .map_err(|_| Error::DigestMismatch)?;
    let canonical_recipe_id = hashv(&[WRAPPER_RECIPE_ID_DOMAIN_V1, &recipe_preimage])?;
    if canonical_recipe_id != wrapper_recipe_id {
        return Err(Error::DigestMismatch);
    }
    let deployment_product_id = canonical_wrapper_product_id_v1(identity, native_claim_id)?;
    hashv(&[
        SERIES_SCOPED_WRAPPER_PRODUCT_DOMAIN_V2,
        &deployment_product_id,
        &structured_root_id,
        &wrapper_recipe_id,
    ])
}

/// Join canonical descriptor semantics to exact deployments, hashes, and PDAs.
#[allow(clippy::too_many_arguments)]
pub fn bind_descriptor_v1<P: PdaVerifierV1>(
    descriptor: StructuredClaimDescriptorV2,
    basis: DescriptorBasisV1,
    deployments: RuntimeDeploymentsV1,
    expected_native_claim_id: Key,
    expected_wrapper_product_id: Key,
    addresses: StructuredClaimRuntimeAddressesV1,
    verifier: &P,
) -> Result<BoundDescriptorV1> {
    deployments.validate()?;
    let identity = reconstruct_descriptor_identity_v1(&descriptor, basis, deployments.binding)?;
    let native_claim_id = canonical_native_claim_id_v1(&identity)?;
    let wrapper_product_id = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )?;
    if native_claim_id != expected_native_claim_id
        || wrapper_product_id != expected_wrapper_product_id
        || is_zero(&native_claim_id)
        || is_zero(&wrapper_product_id)
    {
        return Err(Error::DigestMismatch);
    }
    let address_values = [
        addresses.descriptor,
        addresses.mint,
        addresses.mint_authority,
        addresses.vault_owner,
    ];
    let mut left = 0_usize;
    while left < address_values.len() {
        if is_zero(&address_values[left]) {
            return Err(Error::PdaMismatch);
        }
        let mut right = left + 1;
        while right < address_values.len() {
            if address_values[left] == address_values[right] {
                return Err(Error::PdaMismatch);
            }
            right += 1;
        }
        left += 1;
    }
    let checks = [
        (
            addresses.descriptor,
            DESCRIPTOR_SEED,
            descriptor.descriptor_bump,
        ),
        (addresses.mint, MINT_SEED, descriptor.mint_bump),
        (
            addresses.mint_authority,
            MINT_AUTHORITY_SEED,
            descriptor.mint_authority_bump,
        ),
        (
            addresses.vault_owner,
            VAULT_OWNER_SEED,
            descriptor.vault_owner_bump,
        ),
    ];
    let mut index = 0_usize;
    while index < checks.len() {
        if !verifier.verify(
            &deployments.binding.wrapper_program,
            &checks[index].0,
            checks[index].1,
            &wrapper_product_id,
            checks[index].2,
        ) {
            return Err(Error::PdaMismatch);
        }
        index += 1;
    }
    Ok(BoundDescriptorV1 {
        descriptor,
        identity,
        native_claim_id,
        wrapper_product_id,
        addresses,
    })
}

fn hash(input: &[u8]) -> Result<Key> {
    hashv(&[input])
}

fn hashv(inputs: &[&[u8]]) -> Result<Key> {
    #[cfg(target_os = "solana")]
    {
        Ok(solana_sha256_hasher::hashv(inputs).to_bytes())
    }
    #[cfg(not(target_os = "solana"))]
    {
        let mut hasher = Sha256::new();
        for input in inputs {
            hasher.update(input);
        }
        let digest = hasher.finalize();
        let mut value = [0_u8; 32];
        value.copy_from_slice(&digest);
        Ok(value)
    }
}
