//! Campaign-start assertion: the ACTIVATED release must be the LIVE one.
//!
//! The activation cache is the chain's own statement of which release is in
//! force, and `ArtifactReleaseV1::authenticate_deployment` pins each role's
//! deployment slot and ELF digest against it on chain. So when the programs are
//! upgraded in place — which the permanent-ID ladder does on purpose, keeping
//! every address — and the activated set is not re-activated to follow, the
//! cache silently describes code that is no longer running. Nothing about the
//! account changes: it keeps its Registry owner, its `DCLTACT1` magic and its
//! exact width forever. Only its CONTENT ages.
//!
//! That is not a hypothetical. On 2026-08-29 the shipped devnet client manifest
//! named an activation cache four cohorts stale, whose five pinned deployment
//! slots matched nothing on chain, and it had passed an existence/owner/magic
//! audit that same morning.
//!
//! Downstream, the failure presents as `0x4001` (the `Release` family) from
//! whichever program reauthenticates first — forty accounts deep inside a
//! composed instruction, after several legs have already executed. Diagnosing
//! it that way cost a day and a dozen burned market mints. Named here, at
//! campaign start, it is one sentence.
//!
//! This costs ZERO additional RPC. `campaign::substrate_state` already reads
//! every role's ProgramData and records its live deployment slot and live ELF
//! digest; this compares those against the activation cache the plan derives.
//! It is a supersession detector, deliberately not a full deployment
//! authentication — `campaign::authenticate_checked_live_substrate` does that
//! job, but only for a plan carrying checked deployment evidence, and the
//! supersession class bites plans that carry none.

use dclutch_registry_contract::ActivatedExecutionReleaseSetV1;
use dclutch_release_set_contract::ExecutionRoleV1;

use crate::{Error, Result, campaign::ObservedRoleV1, model::SuccessorPlan, plan::hex, runtime};

/// The five execution roles, with the role names the plan and the observed
/// substrate rows use, in the activation cache's own order.
const ACTIVATED_ROLE_NAMES_V1: [(ExecutionRoleV1, &str); 5] = [
    (ExecutionRoleV1::Core, "core"),
    (ExecutionRoleV1::Claims, "claims"),
    (ExecutionRoleV1::Trading, "trading"),
    (ExecutionRoleV1::Resolution, "resolution"),
    (ExecutionRoleV1::Custody, "custody"),
];

/// Name every way the activated release disagrees with what is actually live.
///
/// Pure: the caller supplies both the activated projection and the rows already
/// read off the cluster. Empty means the activated release IS the live one.
pub(crate) fn activated_release_supersession_v1(
    expected: ActivatedExecutionReleaseSetV1,
    observed: &[ObservedRoleV1],
) -> Vec<String> {
    let mut refusals = Vec::new();
    for (role, name) in ACTIVATED_ROLE_NAMES_V1 {
        let Some(row) = observed.iter().find(|row| row.role == name) else {
            refusals.push(format!(
                "{name}: the activated release binds this role and the substrate reading has no row for it"
            ));
            continue;
        };
        let release = expected.role(role).release();
        let activated_slot = release.deployment_slot();
        match row.observed_slot {
            None => refusals.push(format!(
                "{name}: activation pinned deployment slot {activated_slot}, and ProgramData {} does not exist",
                row.programdata_id
            )),
            Some(live_slot) if live_slot != activated_slot => refusals.push(format!(
                "{name}: activation pinned deployment slot {activated_slot}, live slot is {live_slot}"
            )),
            Some(_) => {}
        }
        let activated_digest = hex(&release.elf_digest());
        if let Some(live_digest) = row.observed_live_elf_sha256.as_deref()
            && live_digest != activated_digest
        {
            refusals.push(format!(
                "{name}: activation pinned live ELF sha256 {activated_digest}, live ELF hashes to {live_digest}"
            ));
        }
    }
    refusals
}

/// Refuse at campaign start when the plan's activated release is not live.
///
/// Names the release set, every disagreeing role, and both sides of each
/// disagreement, so the answer is the refusal rather than the starting point of
/// an investigation.
pub(crate) fn authenticate_activated_release_is_live_v1(
    plan: &SuccessorPlan,
    observed: &[ObservedRoleV1],
) -> Result<()> {
    let expected = runtime::expected_activation(plan)?;
    let refusals = activated_release_supersession_v1(expected, observed);
    if refusals.is_empty() {
        return Ok(());
    }
    let release_set = hex(expected.execution_release_set_id().as_bytes());
    Err(Error::new(format!(
        "the activated release is not the one running on this cluster: activation cache {} \
         (release set {release_set}) has been SUPERSEDED by an in-place upgrade, so every route \
         that reauthenticates a role against it will refuse on chain — {}. Re-activate the \
         release set that matches the live deployment before running this campaign.",
        plan.activation,
        refusals.join("; "),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_registry_contract::{
        ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ARTIFACT_RELEASE_BYTES_V1,
    };
    use sha2::Digest as _;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk_ids::bpf_loader_upgradeable;

    /// One activation cache whose five roles carry the given slots and ELFs.
    fn activated(slots: [u64; 5]) -> (ActivatedExecutionReleaseSetV1, [String; 5]) {
        let loader = bpf_loader_upgradeable::ID;
        let mut cache = [0u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        cache[..8].copy_from_slice(b"DCLTACT1");
        cache[8..10].copy_from_slice(&1u16.to_le_bytes());
        cache[10..12].copy_from_slice(&1u16.to_le_bytes());
        let mut artifacts: Vec<[u8; ARTIFACT_RELEASE_BYTES_V1]> = Vec::new();
        let mut digests: Vec<String> = Vec::new();
        for (index, slot) in slots.into_iter().enumerate() {
            let program = Pubkey::new_from_array([(index as u8) + 1; 32]);
            let programdata = Pubkey::find_program_address(&[program.as_ref()], &loader).0;
            let elf: Vec<u8> = (0..64u8)
                .map(|byte| byte.wrapping_add(slot as u8))
                .collect();
            let elf_digest: [u8; 32] = sha2::Sha256::digest(&elf).into();
            let mut record = [0u8; ARTIFACT_RELEASE_BYTES_V1];
            record[..8].copy_from_slice(b"DCLTARF1");
            record[8..10].copy_from_slice(&1u16.to_le_bytes());
            record[10..12].copy_from_slice(&1u16.to_le_bytes());
            record[16..48].copy_from_slice(&program.to_bytes());
            record[48..80].copy_from_slice(&loader.to_bytes());
            record[80..112].copy_from_slice(&programdata.to_bytes());
            record[112..144].copy_from_slice(&[(index as u8) + 0x40; 32]);
            record[144..176].copy_from_slice(&elf_digest);
            record[176..184].copy_from_slice(&slot.to_le_bytes());
            digests.push(hex(&elf_digest));
            artifacts.push(record);
        }
        // Roles are stored Core, Claims, Trading, Resolution, Custody.
        let mut release_set = [0u8; 336];
        release_set[..8].copy_from_slice(b"DCLTRLS1");
        release_set[8..10].copy_from_slice(&1u16.to_le_bytes());
        release_set[10..12].copy_from_slice(&1u16.to_le_bytes());
        for (index, record) in artifacts.iter().enumerate() {
            let artifact_id: [u8; 32] = sha2::Sha256::digest(record).into();
            release_set[16 + index * 64..48 + index * 64].copy_from_slice(&record[16..48]);
            release_set[48 + index * 64..80 + index * 64].copy_from_slice(&artifact_id);
            let offset = 48 + index * (32 + ARTIFACT_RELEASE_BYTES_V1);
            cache[offset..offset + 32].copy_from_slice(&artifact_id);
            cache[offset + 32..offset + 32 + ARTIFACT_RELEASE_BYTES_V1].copy_from_slice(record);
        }
        let release_set_id: [u8; 32] = sha2::Sha256::digest(release_set).into();
        cache[16..48].copy_from_slice(&release_set_id);
        let decoded = ActivatedExecutionReleaseSetV1::decode(&cache).expect("activation decodes");
        let digests: [String; 5] = digests.try_into().expect("five digests");
        (decoded, digests)
    }

    fn row(role: &str, slot: Option<u64>, elf: Option<String>) -> ObservedRoleV1 {
        ObservedRoleV1 {
            role: role.to_owned(),
            program_id: String::new(),
            programdata_id: "ProgramDataAddress".to_owned(),
            observed_slot: slot,
            pinned_slot: slot.unwrap_or_default(),
            observed_authority: None,
            pinned_authority: None,
            observed_owner: None,
            observed_executable: None,
            observed_live_elf_sha256: elf,
            pinned_live_elf_sha256: String::new(),
            checked_candidate_elf_sha256: String::new(),
            live_elf_padding_bytes: 0,
            observed_data_len: None,
        }
    }

    fn rows(slots: [u64; 5], digests: &[String; 5]) -> Vec<ObservedRoleV1> {
        ACTIVATED_ROLE_NAMES_V1
            .iter()
            .enumerate()
            .map(|(index, (_, name))| row(name, Some(slots[index]), Some(digests[index].clone())))
            .collect()
    }

    #[test]
    fn a_live_activation_names_nothing() {
        let slots = [700, 701, 702, 703, 704];
        let (expected, digests) = activated(slots);
        assert!(activated_release_supersession_v1(expected, &rows(slots, &digests)).is_empty());
    }

    #[test]
    fn an_upgraded_deployment_is_named_role_by_role_with_both_slots() {
        let activated_slots = [700, 701, 702, 703, 704];
        let (expected, activated_digests) = activated(activated_slots);
        // Every role redeployed in place: addresses unchanged, slots moved.
        let live_slots = [900, 901, 902, 903, 904];
        let (_, live_digests) = activated(live_slots);
        let refusals =
            activated_release_supersession_v1(expected, &rows(live_slots, &live_digests));
        // Five slot disagreements and five ELF disagreements, each named.
        assert_eq!(refusals.len(), 10, "{refusals:#?}");
        assert!(
            refusals.iter().any(|refusal| refusal
                == "core: activation pinned deployment slot 700, live slot is 900")
        );
        assert!(
            refusals
                .iter()
                .any(|refusal| refusal.starts_with("custody: activation pinned live ELF sha256"))
        );
        assert_ne!(activated_digests[0], live_digests[0]);
    }

    #[test]
    fn one_moved_role_is_named_and_the_other_four_are_not() {
        let slots = [700, 701, 702, 703, 704];
        let (expected, digests) = activated(slots);
        let mut observed = rows(slots, &digests);
        observed[2].observed_slot = Some(999);
        let refusals = activated_release_supersession_v1(expected, &observed);
        assert_eq!(
            refusals,
            vec!["trading: activation pinned deployment slot 702, live slot is 999".to_owned()]
        );
    }

    #[test]
    fn an_absent_programdata_is_named_rather_than_skipped() {
        let slots = [700, 701, 702, 703, 704];
        let (expected, digests) = activated(slots);
        let mut observed = rows(slots, &digests);
        observed[4].observed_slot = None;
        observed[4].observed_live_elf_sha256 = None;
        let refusals = activated_release_supersession_v1(expected, &observed);
        assert_eq!(refusals.len(), 1, "{refusals:#?}");
        assert!(refusals[0].contains("custody"));
        assert!(refusals[0].contains("does not exist"));
    }

    #[test]
    fn a_missing_substrate_row_refuses_instead_of_passing_vacuously() {
        let slots = [700, 701, 702, 703, 704];
        let (expected, digests) = activated(slots);
        let observed: Vec<ObservedRoleV1> = rows(slots, &digests)
            .into_iter()
            .filter(|row| row.role != "resolution")
            .collect();
        let refusals = activated_release_supersession_v1(expected, &observed);
        assert_eq!(refusals.len(), 1, "{refusals:#?}");
        assert!(refusals[0].contains("resolution"));
        assert!(refusals[0].contains("no row for it"));
    }
}
