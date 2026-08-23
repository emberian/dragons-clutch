//! Native-claim, deployment, product-digest, and PDA binding.

use clutch_solana_layout::{
    portfolio_settlement::NativePortfolioClaimV1, Hash32, MarketAccount, TermsAccount,
};
use clutch_structured_claim::{ClaimVector, DeploymentBinding, NativeBasisIdentity, NativeClaim};
#[cfg(not(target_os = "solana"))]
use sha2::{Digest, Sha256};

use crate::{codec::DESCRIPTOR_LIVE, is_zero, Error, Key, Result};
use crate::{StructuredClaimDescriptorV1, MAX_OUTCOMES};

/// Descriptor PDA seed prefix.
pub const DESCRIPTOR_SEED: &[u8] = b"dc:claim-desc:v1";
/// Wrapper mint PDA seed prefix.
pub const MINT_SEED: &[u8] = b"dc:claim-mint:v1";
/// Shared mint-authority and base-Position owner PDA seed prefix.
pub const VAULT_OWNER_SEED: &[u8] = b"dc:claim-vault:v1";
/// Per-actor wrapper replay PDA seed prefix.
pub const REPLAY_SEED: &[u8] = b"dc:claim-replay:v1";

/// Authenticated executable and upgradeable-loader observations.
///
/// The owning dispatcher decodes the executable and ProgramData accounts into
/// this projection. The three slots and six identities become part of product
/// fungibility; a deployment change therefore fails closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeDeployments {
    /// Exact identities and deployment slots observed from the three programs.
    pub binding: DeploymentBinding,
    /// Pinned upgradeable-loader executable.
    pub upgradeable_loader: Key,
    /// Owner of each Program account: wrapper, base, then Token-2022.
    pub program_owners: [Key; 3],
    /// Owner of each ProgramData account in the same order.
    pub program_data_owners: [Key; 3],
    /// ProgramData address named by each Program account in the same order.
    pub linked_program_data: [Key; 3],
    /// Bit `i` states that program `i` is executable; exactly `0b111` is valid.
    pub executable_mask: u8,
}

impl RuntimeDeployments {
    /// Validate loader ownership, Program→ProgramData linkage, and identities.
    pub fn validate(&self) -> Result<()> {
        self.binding
            .validate()
            .map_err(|_| Error::DeploymentMismatch)?;
        if is_zero(&self.upgradeable_loader) || self.executable_mask != 0b111 {
            return Err(Error::DeploymentMismatch);
        }
        let expected_data = [
            self.binding.wrapper_program_data,
            self.binding.base_program_data,
            self.binding.token_2022_program_data,
        ];
        let mut i = 0;
        while i < 3 {
            if self.program_owners[i] != self.upgradeable_loader
                || self.program_data_owners[i] != self.upgradeable_loader
                || self.linked_program_data[i] != expected_data[i]
            {
                return Err(Error::DeploymentMismatch);
            }
            i += 1;
        }
        Ok(())
    }
}

/// Canonical wrapper-owned account addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressBinding {
    /// Descriptor account.
    pub descriptor: Key,
    /// Extension-free Token-2022 mint.
    pub mint: Key,
    /// Mint authority and owner of the dedicated base Position.
    pub vault_owner: Key,
}

impl AddressBinding {
    fn validate_shape(&self) -> Result<()> {
        let values = [self.descriptor, self.mint, self.vault_owner];
        let mut left = 0;
        while left < values.len() {
            if is_zero(&values[left]) {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < values.len() {
                if values[left] == values[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }
}

/// Runtime program-address verifier.
///
/// Production uses `create_program_address` with the exact prefix, product id,
/// and persisted bump. Host tests inject a deterministic verifier because the
/// repository intentionally does not link a host curve backend.
pub trait PdaVerifier {
    /// Return true only if `address` is exactly the PDA for the given tuple.
    fn verify(
        &self,
        program: &Key,
        address: &Key,
        prefix: &[u8],
        product_id: &Key,
        bump: u8,
    ) -> bool;
}

/// Onchain verifier backed by Solana's program-address syscall.
#[cfg(target_os = "solana")]
#[derive(Clone, Copy, Debug, Default)]
pub struct SolanaPdaVerifier;

#[cfg(target_os = "solana")]
impl PdaVerifier for SolanaPdaVerifier {
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
        match Pubkey::create_program_address(&[prefix, product_id, &bump_seed], &program) {
            Ok(derived) => derived.to_bytes() == *address,
            Err(_) => false,
        }
    }
}

/// SHA-256 of the exact core-owned wrapper-product preimage.
pub fn canonical_wrapper_product_id(
    deployments: &DeploymentBinding,
    native_claim_id: Key,
) -> Result<Key> {
    let preimage = deployments.product_preimage(native_claim_id)?;
    #[cfg(target_os = "solana")]
    {
        Ok(solana_sha256_hasher::hash(&preimage).to_bytes())
    }
    #[cfg(not(target_os = "solana"))]
    {
        let digest = Sha256::digest(preimage);
        let mut output = [0; 32];
        output.copy_from_slice(&digest);
        Ok(output)
    }
}

/// Fixed replay namespace binding one wrapper product and one actor.
pub fn canonical_replay_namespace(product_id: Key, actor: Key) -> Result<Key> {
    if is_zero(&product_id) || is_zero(&actor) {
        return Err(Error::InvalidIdentity);
    }
    #[cfg(target_os = "solana")]
    {
        Ok(
            solana_sha256_hasher::hashv(&[
                b"dragons-clutch/wrapper-replay/v1",
                &product_id,
                &actor,
            ])
            .to_bytes(),
        )
    }
    #[cfg(not(target_os = "solana"))]
    {
        let mut hasher = Sha256::new();
        hasher.update(b"dragons-clutch/wrapper-replay/v1");
        hasher.update(product_id);
        hasher.update(actor);
        Ok(hasher.finalize().into())
    }
}

/// Bind a descriptor to live Market/Terms, three deployments, product digest,
/// and wrapper PDAs, returning the core-owned native claim.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn bind_descriptor<P: PdaVerifier>(
    descriptor: &StructuredClaimDescriptorV1,
    wrapper_program: Key,
    market: &MarketAccount,
    terms: &TermsAccount,
    deployments: &RuntimeDeployments,
    expected_native_claim_id: Key,
    expected_product_id: Key,
    addresses: &AddressBinding,
    verifier: &P,
) -> Result<NativeClaim> {
    descriptor.validate_shape()?;
    deployments.validate()?;
    addresses.validate_shape()?;
    market.validate().map_err(|_| Error::DescriptorBinding)?;
    terms
        .binds_market(market)
        .map_err(|_| Error::DescriptorBinding)?;
    if descriptor.state > DESCRIPTOR_LIVE + 1
        || descriptor.market != market.market.bytes()
        || descriptor.terms != terms.terms.bytes()
        || is_zero(&wrapper_program)
    {
        return Err(Error::DescriptorBinding);
    }

    let observed = deployments.binding;
    if observed.wrapper_program != wrapper_program
        || observed.base_program != descriptor.base_program
        || observed.base_program_data != descriptor.base_program_data
        || observed.base_deployment_slot != descriptor.base_deployment_slot
        || observed.wrapper_program_data != descriptor.wrapper_program_data
        || observed.wrapper_deployment_slot != descriptor.wrapper_deployment_slot
        || observed.token_2022_program != descriptor.token_2022_program
        || observed.token_2022_program_data != descriptor.token_2022_program_data
        || observed.token_2022_deployment_slot != descriptor.token_2022_deployment_slot
    {
        return Err(Error::DeploymentMismatch);
    }

    let (live_claim, removed_gcd) = NativePortfolioClaimV1::compile(
        Hash32::from_bytes(descriptor.market),
        terms,
        descriptor.primitive,
    )
    .map_err(|_| Error::DescriptorBinding)?;
    if removed_gcd != 1 || live_claim.claim.bytes() != expected_native_claim_id {
        return Err(Error::DigestMismatch);
    }

    let vector = ClaimVector {
        outcome_count: terms.outcome_count,
        coefficients: descriptor.primitive,
    };
    let claim = NativeClaim {
        basis: NativeBasisIdentity {
            market: descriptor.market,
            terms: descriptor.terms,
            basis_degree: terms.basis_degree,
            denominator: terms.payouts[0].denominator,
            outcome_count: terms.outcome_count,
        },
        vector,
    };
    claim.validate()?;
    // The two owners emit byte-identical native claim preimages; the live
    // layout digest above is the runtime authority and this comparison guards
    // accidental drift in fields before product hashing.
    if claim.identity_preimage().is_err() {
        return Err(Error::DigestMismatch);
    }
    let product_id = canonical_wrapper_product_id(&observed, expected_native_claim_id)?;
    if product_id != expected_product_id {
        return Err(Error::DigestMismatch);
    }

    let checks = [
        (
            addresses.descriptor,
            DESCRIPTOR_SEED,
            descriptor.descriptor_bump,
        ),
        (addresses.mint, MINT_SEED, descriptor.mint_bump),
        (
            addresses.vault_owner,
            VAULT_OWNER_SEED,
            descriptor.vault_owner_bump,
        ),
    ];
    let mut i = 0;
    while i < checks.len() {
        if !verifier.verify(
            &wrapper_program,
            &checks[i].0,
            checks[i].1,
            &product_id,
            checks[i].2,
        ) {
            return Err(Error::PdaMismatch);
        }
        i += 1;
    }
    Ok(claim)
}

const _: () = assert!(MAX_OUTCOMES == clutch_solana_layout::MAX_OUTCOMES);
