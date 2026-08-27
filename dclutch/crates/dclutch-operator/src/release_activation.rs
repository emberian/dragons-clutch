//! Checked-release admission into the existing unsigned Registry workflow.
//!
//! This module is a host-only join, not a second release authority. It proves
//! that one release-tool manifest, its five complete checked releases, the
//! finalized Registry records, and the observed Loader V3 deployments all
//! describe the same release set. Only then does it return the existing
//! Registry activation instruction and packet plan.

use dclutch_release_set_contract::{EXECUTION_ROLE_COUNT_V1, ExecutionRoleV1};
use dclutch_release_tool::{
    CheckedExecutionReleaseSetV1, CheckedReleaseV1, build_checked_execution_release_set,
};
use solana_hash::Hash;
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::registry::{
    Error as RegistryError, RegistryActivationReport, RegistryActivationState,
    RegistryPacketPlanV0, build_registry_activation_v1, compile_registry_role_activation_packet_v0,
};

const ROLES: [ExecutionRoleV1; EXECUTION_ROLE_COUNT_V1] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

/// A checked five-program release joined to one existing unsigned activation.
///
/// `checked_release_set` remains the canonical offline evidence value;
/// `activation` remains the canonical Registry operator result. This wrapper
/// deliberately persists no parallel copy of either authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryActivationPlanV1 {
    /// Canonical checked multiprogram evidence admitted by this plan.
    pub checked_release_set: CheckedExecutionReleaseSetV1,
    /// Existing chain-derived Registry activation report.
    pub activation: RegistryActivationReport,
    /// Five ordered packet-safe unsigned v0 message plans, one per role.
    ///
    /// Activation admits one role per transaction; see
    /// [`RegistryActivationReport::roles`].
    pub packets: [RegistryPacketPlanV0; EXECUTION_ROLE_COUNT_V1],
}

impl CheckedRegistryActivationPlanV1 {
    /// Render a deterministic, line-oriented evidence projection.
    ///
    /// The projection is explicitly non-authoritative: every value is derived
    /// from the canonical checked manifest, Registry report, and unsigned
    /// message already held by this plan.
    pub fn render_evidence_text(&self) -> Result<String, Error> {
        let mut output = self
            .checked_release_set
            .render_text()
            .map_err(Error::ReleaseTool)?;
        push_line(&mut output, "activation_projection", "dclutch-registry-v1");
        push_line(
            &mut output,
            "observation_slot",
            &self.activation.observation.slot.to_string(),
        );
        push_line(
            &mut output,
            "observation_unix_timestamp",
            &self.activation.observation.unix_timestamp.to_string(),
        );
        push_line(&mut output, "observation_finality", "finalized");
        push_line(
            &mut output,
            "registry_program_id",
            &self
                .activation
                .roles
                .first()
                .ok_or(Error::IdentityMismatch)?
                .instruction
                .program_id
                .to_string(),
        );
        push_line(
            &mut output,
            "activation_cache",
            &self.activation.cache.to_string(),
        );
        push_line(
            &mut output,
            "activation_mode",
            &match self.activation.mode {
                crate::registry::RegistryActivationModeV1::Create => String::from("create"),
                crate::registry::RegistryActivationModeV1::Partial { activated_roles } => {
                    format!("partial:{activated_roles}")
                }
                crate::registry::RegistryActivationModeV1::Repeat => String::from("repeat"),
            },
        );
        push_line(
            &mut output,
            "cache_rent_debit_lamports",
            &self.activation.cache_rent_debit_lamports.to_string(),
        );
        push_line(
            &mut output,
            "elf_bytes_hashed_total",
            &self.activation.compute.elf_bytes_hashed.to_string(),
        );
        push_line(
            &mut output,
            "activation_transactions",
            &self.activation.roles.len().to_string(),
        );
        for plan in &self.activation.roles {
            push_line(
                &mut output,
                &format!("role_elf_bytes_hashed_{}", role_label(plan.role)),
                &plan.compute.elf_bytes_hashed.to_string(),
            );
        }
        push_optional_u32(
            &mut output,
            "matching_measured_compute_units",
            self.activation.compute.matching_measured_compute_units,
        );
        for (plan, packet) in self.activation.roles.iter().zip(self.packets.iter()) {
            let label = role_label(plan.role);
            push_line(
                &mut output,
                &format!("unsigned_message_sha256_{label}"),
                &hex(&hash(&packet.message.serialize()).to_bytes()),
            );
            push_line(
                &mut output,
                &format!("packet_wire_bytes_{label}"),
                &packet.wire_bytes.to_string(),
            );
            push_line(
                &mut output,
                &format!("required_signatures_{label}"),
                &packet.required_signatures.to_string(),
            );
            push_line(
                &mut output,
                &format!("compute_unit_limit_{label}"),
                &packet.compute_unit_limit.to_string(),
            );
            push_optional_u32(
                &mut output,
                &format!("measured_headroom_{label}"),
                packet.measured_headroom,
            );
        }
        Ok(output)
    }
}

/// Refusal from the checked-release, Registry-admission, or identity join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The complete checked-release evidence was malformed or inconsistent.
    ReleaseTool(dclutch_release_tool::Error),
    /// The existing chain-derived Registry builder refused the observation.
    Registry(RegistryError),
    /// Checked evidence and finalized Registry/Loader authority were not exact.
    IdentityMismatch,
}

/// Join checked multiprogram evidence to one packet-safe Registry activation.
///
/// `checked_releases` are required even though their content identities occur
/// in `checked_release_set`: rebuilding the latter prevents a caller from
/// merely decoding a self-consistent envelope containing invented checked-
/// release IDs. No RPC, signing, submission, or account mutation occurs.
pub fn build_checked_registry_activation_packet_v1(
    registry_program: Pubkey,
    checked_release_set: CheckedExecutionReleaseSetV1,
    checked_releases: [&CheckedReleaseV1; EXECUTION_ROLE_COUNT_V1],
    state: &RegistryActivationState,
    fee_payer: Pubkey,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
) -> Result<CheckedRegistryActivationPlanV1, Error> {
    let rebuilt =
        build_checked_execution_release_set(checked_release_set.release_set(), checked_releases)
            .map_err(Error::ReleaseTool)?;
    if rebuilt != checked_release_set {
        return Err(Error::IdentityMismatch);
    }

    let activation =
        build_registry_activation_v1(registry_program, state).map_err(Error::Registry)?;
    let activated_release_set = activation
        .expected_cache
        .release_set_projection()
        .map_err(|_| Error::IdentityMismatch)?;
    if activated_release_set != checked_release_set.release_set()
        || activation.execution_release_set_id
            != checked_release_set
                .execution_release_set_id()
                .map_err(Error::ReleaseTool)?
    {
        return Err(Error::IdentityMismatch);
    }

    for ((role, checked_artifact), checked_release) in ROLES
        .into_iter()
        .zip(checked_release_set.artifacts())
        .zip(checked_releases)
    {
        let activated = activation.expected_cache.role(role);
        if activated.release() != checked_artifact
            || checked_artifact.program().to_bytes() != checked_release.program_id()
            || checked_artifact.programdata() != checked_release.programdata_id()
            || checked_artifact.loader_program().to_bytes() != checked_release.loader_program_id()
            || checked_artifact.semantic_release_id() != checked_release.semantic_release_id()
            || checked_artifact.elf_digest() != checked_release.artifact_digest()
            || checked_artifact.deployment_slot() != checked_release.deployment_slot()
            || checked_artifact.upgrade_authority() != checked_release.upgrade_authority()
        {
            return Err(Error::IdentityMismatch);
        }
    }

    let mut compiled = Vec::with_capacity(EXECUTION_ROLE_COUNT_V1);
    for plan in &activation.roles {
        compiled.push(
            compile_registry_role_activation_packet_v0(
                plan,
                fee_payer,
                recent_blockhash,
                compute_unit_limit,
            )
            .map_err(Error::Registry)?,
        );
    }
    let packets: [RegistryPacketPlanV0; EXECUTION_ROLE_COUNT_V1] =
        compiled.try_into().map_err(|_| Error::IdentityMismatch)?;
    Ok(CheckedRegistryActivationPlanV1 {
        checked_release_set,
        activation,
        packets,
    })
}

const fn role_label(role: ExecutionRoleV1) -> &'static str {
    match role {
        ExecutionRoleV1::Core => "core",
        ExecutionRoleV1::Claims => "claims",
        ExecutionRoleV1::Trading => "trading",
        ExecutionRoleV1::Resolution => "resolution",
        ExecutionRoleV1::Custody => "custody",
    }
}

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

fn push_optional_u32(output: &mut String, key: &str, value: Option<u32>) {
    match value {
        Some(value) => push_line(output, key, &value.to_string()),
        None => push_line(output, key, "none"),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        let high = DIGITS.get(usize::from(byte >> 4)).copied().unwrap_or(b'?');
        let low = DIGITS
            .get(usize::from(byte & 0x0f))
            .copied()
            .unwrap_or(b'?');
        output.push(char::from(high));
        output.push(char::from(low));
    }
    output
}

#[cfg(test)]
mod tests;
