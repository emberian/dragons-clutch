//! Fail-closed dispatch coordinator for the future main-program route arm.

use crate::runtime_contract::{StructuredClaimActionV1, StructuredClaimPayloadV1};
use crate::{admit_runtime_envelope_v1, AccountFrameV1, PreparedStructuredClaimRouteV1, Result};

/// Account loader/executor-plan constructor owned by the future main SBF seam.
///
/// An implementation must validate the exact [`AccountFrameV1`] for `action`,
/// decode every hostile base/descriptor/Token-2022 account through this
/// adapter's authenticators, and call `prepare_create_descriptor_v1` or
/// `prepare_mutation_v1`. It must not perform a CPI or write before returning
/// the fully staged plan.
pub trait StructuredClaimAccountLoaderV1 {
    /// Load authoritative accounts and prepare one exact route.
    fn load_and_prepare(
        &self,
        action: StructuredClaimActionV1,
        payload: StructuredClaimPayloadV1,
        accounts: &AccountFrameV1<'_>,
    ) -> Result<PreparedStructuredClaimRouteV1>;
}

/// Admit, decode, load, and stage one structured-claim family instruction.
///
/// With the current empty capability mask, this returns
/// `Error::CapabilityDisabled` after inspecting only the three-byte family
/// header. `accounts` and `loader` are not accessed. A future activation must
/// change the mask only together with the central capability tuple and exact
/// dispatcher/account-loader implementation.
pub fn dispatch_structured_claim_v1<L: StructuredClaimAccountLoaderV1>(
    family_instruction: &[u8],
    accounts: &AccountFrameV1<'_>,
    loader: &L,
) -> Result<PreparedStructuredClaimRouteV1> {
    let envelope = admit_runtime_envelope_v1(family_instruction)?;
    let payload = envelope.decode_payload()?;
    loader.load_and_prepare(envelope.action, payload, accounts)
}
