//! What the DEPLOYED Core writes into a Market's reserved bump tail.
//!
//! The founding predictor is not a description of this tree. It is a MODEL OF
//! THE PROGRAM THAT IS RUNNING, because it commits to `sha256(CoreState)` two
//! stages before Core writes one: the candidate state is hashed into the
//! projected Realize receipt whose digest reaches the permit through
//! `FoundingIntentV5`. A model that describes the wrong build produces a permit
//! the chain refuses, and it refuses LATE.
//!
//! `b312ce3c4` (2026-09-03 00:51) made Core record the Product graph's eight
//! bumps in four of the Market's five reserved bytes. Cohort-14's deployed
//! bytes are `8e96ec3f8` (2026-09-02 19:44), and
//! `git merge-base --is-ancestor b312ce3c4 8e96ec3f8` is false: that Core
//! writes zeros there. Cohort-14c measured the cost of the host predicting
//! bumps against it anyway — the founding refused
//!
//! > the Market the chain holds carries bump tail StateBumpsV1 { ...,
//! > product_graph: ProductGraphBumpsV1([0, 0, 0, 0, 0, 0, 0, 0]) }, and this
//! > driver predicts ProductGraphBumpsV1([254, 255, 255, 255, 255, 255, 253,
//! > 255])
//!
//! **after 0.139 SOL and after the canonical Found37 Market existed**. Nothing
//! in that refusal needed a transaction to discover. Which build is deployed is
//! a fact the plan carries and the substrate reading confirms, so the answer is
//! available at campaign start, for free, before a lamport moves.
//!
//! Two questions, and they are not the same question — the shape
//! `collateral_release.rs` already uses for founded-versus-admitted:
//!
//!   * WHAT DOES THIS TREE'S CORE WRITE. [`CoreProductGraphProjectionV1::Recorded`],
//!     always, since `b312ce3c4`. It is not a table entry and never needs one.
//!   * WHAT DOES THE DEPLOYED CORE WRITE. Whatever build is on the cluster,
//!     which may predate this tree by any amount. That one is answered from the
//!     deployment's own identity — its ELF digest — and never from what this
//!     source file happens to do today.

use crate::{
    Error, Result,
    campaign::ObservedRoleV1,
    model::{CheckedLocalMutableSetPinV1, ProgramPin},
};

/// The Core role's name in a plan and in a substrate reading.
const CORE_ROLE_NAME_V1: &str = "core";

/// A plan pin whose deployment is bytes the chain was already holding.
///
/// The other value, `"genesis-install"`, describes an ELF this run installs
/// itself out of the plan's own checked candidate — which is this tree's build,
/// checked fresh by `tools/release/check_sbf_build_freshness.py` before the
/// release gate will certify it.
const OBSERVED_DEPLOYMENT_SOURCE_V1: &str = "observed-programdata-account";

/// What a deployed Core's `found` leaves in the Product-graph bump nibbles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreProductGraphProjectionV1 {
    /// `found` fills all eight from the walk it performed (`b312ce3c4` on).
    Recorded,
    /// `found` never wrote there; the four reserved bytes are still zero, which
    /// `ProductGraphBumpsV1::ABSENT` is the encoding of.
    Unrecorded,
}

/// Core deployments whose `found` leaves the Product-graph nibbles zero.
///
/// Keyed by the CHECKED CANDIDATE ELF digest — the exact raw build output the
/// release gate certifies and the deploy runbook records — because a
/// deployment's identity is its bytes and nothing else. Append-only: a row here
/// describes a cohort that exists, and deleting one would silently re-arm the
/// late refusal for whoever founds against it next.
const UNRECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1: [&str; 1] = [
    // cohort-14, built at 8e96ec3f8cd4438040d9287d2489ea84587ebd5c, 1,186,424
    // bytes, deployed to devnet at slot 492,226,262 as
    // 9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB. Three markets stand on it.
    "864394530f37c04e53d10f918c8fab0c265187549895bf5a9207ae91f2a7d02f",
];

/// Core deployments whose `found` records all eight.
///
/// A plan that INSTALLS this tree's own checked candidate needs no row, because
/// those bytes are the ones beside this file. This list is the other case — a
/// cohort deployed from a commit that is not the one you are standing on — and
/// it is keyed by BYTES, which is why a row can be written the moment the
/// candidate is built rather than after the deploy has been paid for. That
/// ordering is the point: the refusal this list answers arrives at the FIRST
/// founding, after the redeploy has spent, so a cohort whose row is written
/// afterwards has already bought the discovery it was meant to avoid.
const RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1: [&str; 2] = [
    // cohort-16, 1,186,440 bytes, deployed to devnet as
    // 4wv7JxoAad6JMQi2vHJyByLXasWS8RzJSTdvEEmpCjpe. The deploy commit is
    // f2ae6bf75deb1f71465e1ff06a05abe80d4d22a4 and the bytes are the NAMED
    // RELEASE BUILDER's -- platform-tools v1.53 on Linux/x86_64, built on hbox
    // through swarm-build -- which is why this digest is not the one a macOS
    // build of the same commit produces. `git merge-base --is-ancestor
    // b312ce3c4 f2ae6bf75` is TRUE, so this Core fills all eight nibbles.
    "f637e5df9ef9a16521d58aee080b951f6950f2661abbf2ff573cf7bc98d5665e",
    // cohort-15, 1,193,400 bytes, deployed to devnet as
    // 7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr. The deploy commit is
    // 1cae26fd61defbefd20bcd52acc449b6e94e64ed; these exact bytes also build at
    // its parent 8599cfc69, which is measured rather than assumed -- two
    // detached worktrees, one per commit, produced all seven role ELFs
    // byte-identically, so the lockfile row between them moved no shipped byte.
    // The claim this row makes is a property of the bytes and is checkable with
    // no chain at all: `git merge-base --is-ancestor b312ce3c4 1cae26fd6` is
    // TRUE, so this Core fills all eight nibbles where cohort-14's writes
    // zeros.
    "a5385ba240b6eeae2cb3279d1014690cc44bf87e593dc9b0dc51b974142c73b6",
];

/// The projection recorded for one Core ELF digest, if this driver states one.
fn declared_projection_v1(elf_sha256: &str) -> Option<CoreProductGraphProjectionV1> {
    if elf_sha256.is_empty() {
        return None;
    }
    if UNRECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1.contains(&elf_sha256) {
        return Some(CoreProductGraphProjectionV1::Unrecorded);
    }
    if RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1.contains(&elf_sha256) {
        return Some(CoreProductGraphProjectionV1::Recorded);
    }
    None
}

/// The digest that names a Core deployment, in the order the plan spells it.
///
/// `elf_sha256` is documented as a compatibility alias for
/// `checked_candidate_elf_sha256` and is always exact, so either answers the
/// question; the candidate field is preferred because it is the one the release
/// gate and the runbook both print.
fn pin_elf_sha256_v1(pin: &ProgramPin) -> &str {
    if pin.checked_candidate_elf_sha256.is_empty() {
        &pin.elf_sha256
    } else {
        &pin.checked_candidate_elf_sha256
    }
}

/// Did THIS RUN put those bytes on that chain.
///
/// The exemption above reads `deployment_source`, and its comment names
/// `"genesis-install"` as "every loopback and local-validator run". That is not
/// what the tree does. `local_mutable.rs` REQUIRES every local-mutable pin to
/// spell its source `observed-programdata-account`, because the profile
/// installs the seven checked ELFs into genesis and then authenticates them by
/// reading the ProgramData accounts back -- so the label says "bytes the chain
/// was already holding" about bytes this run put there thirty seconds ago, and
/// the exemption never once fired for the only loopback lifecycle in this tree.
/// Measured 2026-09-03 on a cold machine: `private-validator-lifecycle` refused
/// at the founding stage on a Core whose live ProgramData hashed to the exact
/// checked candidate named in its own plan, five stages after the validator
/// started and with `RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1` empty --- and
/// the digest the refusal asks for is HOST-DEPENDENT (platform-tools embeds its
/// own CI build path in the standard library, so a Linux and a macOS build of
/// one commit have different Core digests), so hand-recording it would need one
/// row per commit per builder OS forever.
///
/// The narrow question this answers instead: is the Core the chain is running
/// the checked candidate THIS PLAN carries, installed by THIS run's own
/// local-mutable genesis. Only the local-mutable profile emits that pin; a
/// devnet plan carries none, so a deployed cohort the driver has no statement
/// about still refuses exactly as before.
fn installed_into_this_runs_genesis_v1(
    core: &ProgramPin,
    local_mutable: Option<&CheckedLocalMutableSetPinV1>,
) -> bool {
    let Some(set) = local_mutable else {
        return false;
    };
    let digest = pin_elf_sha256_v1(core);
    if digest.is_empty() {
        return false;
    }
    set.roles.iter().any(|role| {
        role.role == CORE_ROLE_NAME_V1
            && role.program_id == core.program_id
            && role.checked_candidate_elf_sha256 == digest
            && role.live_elf_sha256 == digest
    })
}

/// What the Core a plan names writes into the reserved bump tail.
///
/// The founding calls this, not the campaign preflight, so a founding driven
/// past the preflight still cannot project the wrong tail. It reads only the
/// plan and costs no RPC.
///
/// The default is deliberate and narrow. A pin that INSTALLS an ELF —
/// `"genesis-install"`, every loopback and local-validator run — is installing
/// the checked candidate this tree built, so it gets this tree's answer. A pin
/// that OBSERVED a ProgramData account is describing bytes that were deployed
/// at some other time, by some other commit, and for those the driver either
/// has a recorded statement or has none; having none is a refusal, because the
/// alternative is exactly the guess cohort-14c paid 0.139 SOL to disprove.
pub(crate) fn core_product_graph_projection_v1(
    core: &ProgramPin,
    local_mutable: Option<&CheckedLocalMutableSetPinV1>,
) -> Result<CoreProductGraphProjectionV1> {
    let digest = pin_elf_sha256_v1(core);
    if let Some(projection) = declared_projection_v1(digest) {
        return Ok(projection);
    }
    if core.deployment_source != OBSERVED_DEPLOYMENT_SOURCE_V1 {
        return Ok(CoreProductGraphProjectionV1::Recorded);
    }
    if installed_into_this_runs_genesis_v1(core, local_mutable) {
        return Ok(CoreProductGraphProjectionV1::Recorded);
    }
    Err(Error::new(format!(
        "this driver cannot say what the DEPLOYED Core writes into the Market's reserved bump \
         tail, so it will not project one: Core {} was observed on chain carrying checked \
         candidate ELF sha256 {digest}, and that digest is in neither \
         UNRECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1 nor \
         RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1. The founding commits to sha256(CoreState) \
         two stages before Core writes one, so a guess here refuses AFTER the Market exists and \
         after the founding has spent. Answer `git merge-base --is-ancestor b312ce3c4 \
         <the revision those bytes were built from>` and add the digest to the list it names.",
        core.program_id,
    )))
}

/// Refuse at campaign start when the host cannot model the deployed Core.
///
/// The plan-time shape `e615593fc` gave the Pyth redeploy check and
/// `release_identity` gave release supersession: a question that needs no
/// transaction, asked before any transaction, answered out of readings the
/// campaign has already taken. It costs no extra RPC — `substrate_state` has
/// already read every role's live ELF digest.
///
/// Two conjuncts, and the first is what makes the second a reading rather than
/// a document check: the Core the CHAIN is running must be the Core the plan
/// describes, and the driver must have a statement about it.
pub(crate) fn authenticate_core_bump_projection_v1(
    core: &ProgramPin,
    observed: &[ObservedRoleV1],
    local_mutable: Option<&CheckedLocalMutableSetPinV1>,
) -> Result<CoreProductGraphProjectionV1> {
    let Some(row) = observed.iter().find(|row| row.role == CORE_ROLE_NAME_V1) else {
        return Err(Error::new(
            "the substrate reading has no Core row, so the bump-tail projection has nothing to \
             reconcile against; refusing before any transaction",
        ));
    };
    if let Some(live) = row.observed_live_elf_sha256.as_deref()
        && !core.live_elf_sha256.is_empty()
        && live != core.live_elf_sha256
    {
        return Err(Error::new(format!(
            "the Core running on this cluster is not the Core this plan describes, so the host's \
             bump-tail projection cannot be reconciled with it: plan pins live ELF sha256 {}, the \
             chain's ProgramData hashes to {live}. Refusing before any transaction.",
            core.live_elf_sha256,
        )));
    }
    core_product_graph_projection_v1(core, local_mutable)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cohort-14's Core, byte for byte off the release gate's own table.
    ///
    /// Written as a literal rather than read from the list under test: derived
    /// from the list it would move with the list and prove nothing.
    const COHORT_14_CORE_ELF_SHA256: &str =
        "864394530f37c04e53d10f918c8fab0c265187549895bf5a9207ae91f2a7d02f";

    fn core_pin(checked_candidate_elf_sha256: &str, deployment_source: &str) -> ProgramPin {
        ProgramPin {
            program_id: "9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB".into(),
            programdata_id: "CC39Q4RstSZBniSZZASYoZXMyQtTari3WHZs9Zscgt2t".into(),
            elf_path: String::new(),
            elf_sha256: checked_candidate_elf_sha256.into(),
            checked_candidate_elf_path: String::new(),
            checked_candidate_elf_sha256: checked_candidate_elf_sha256.into(),
            live_elf_sha256: String::new(),
            live_elf_padding_bytes: 0,
            semantic_release_id: String::new(),
            artifact_release_id: String::new(),
            upgrade_authority: None,
            deployment_slot: 492_226_262,
            deployment_source: deployment_source.into(),
            programdata_sha256: String::new(),
        }
    }

    /// THE COHORT-14C REFUSAL, ASKED BEFORE THE SPEND. A plan naming the Core
    /// that is actually deployed gets the projection that Core actually makes.
    #[test]
    fn a_plan_naming_cohort_14s_core_projects_an_unrecorded_product_graph() {
        assert_eq!(
            core_product_graph_projection_v1(
                &core_pin(COHORT_14_CORE_ELF_SHA256, OBSERVED_DEPLOYMENT_SOURCE_V1),
                None
            )
            .expect("cohort-14's Core is a build this driver has a statement about"),
            CoreProductGraphProjectionV1::Unrecorded,
        );
    }

    /// AND A PLAN CARRYING THIS TREE'S OWN CORE PROJECTS THE BUMPS. Every
    /// loopback and local-validator plan installs the checked candidate built
    /// beside this file, and `b312ce3c4` is in it.
    #[test]
    fn a_plan_installing_this_trees_core_projects_a_recorded_product_graph() {
        assert_eq!(
            core_product_graph_projection_v1(
                &core_pin(
                    "3ba9910250000000000000000000000000000000000000000000000000000000",
                    "genesis-install",
                ),
                None
            )
            .expect("an installed checked candidate is this tree's Core"),
            CoreProductGraphProjectionV1::Recorded,
        );
    }

    /// The digest a cohort-14 build carries is the answer wherever it appears:
    /// installing those exact bytes locally gets the same statement, because
    /// the statement is about the bytes and not about how they arrived.
    #[test]
    fn the_statement_follows_the_bytes_and_not_the_deployment_source() {
        assert_eq!(
            core_product_graph_projection_v1(
                &core_pin(COHORT_14_CORE_ELF_SHA256, "genesis-install"),
                None
            )
            .expect("a recorded digest is recorded however it was installed"),
            CoreProductGraphProjectionV1::Unrecorded,
        );
    }

    /// The local-mutable set pin, as the private-validator lifecycle emits it.
    fn local_mutable_set(
        program_id: &str,
        digest: &str,
    ) -> crate::model::CheckedLocalMutableSetPinV1 {
        crate::model::CheckedLocalMutableSetPinV1 {
            schema: "dclutch-checked-local-mutable-set-v1".into(),
            checked_release_gate_path: String::new(),
            checked_release_gate_sha256: String::new(),
            source_revision: String::new(),
            source_tree_sha256: String::new(),
            solana_cli_version: String::new(),
            retained_upgrade_authority: String::new(),
            execution_release_set: crate::model::CheckedLocalExecutionReleaseSetPinV1 {
                schema: String::new(),
                checked_execution_release_set_id: String::new(),
                execution_release_set_id: String::new(),
                checked_execution_release_set_base64: String::new(),
                roles: Vec::new(),
            },
            set_sha256: String::new(),
            roles: vec![crate::model::CheckedLocalMutableRolePinV1 {
                role: CORE_ROLE_NAME_V1.into(),
                program_id: program_id.into(),
                programdata_id: String::new(),
                checked_candidate_elf_path: String::new(),
                checked_candidate_elf_sha256: digest.into(),
                live_elf_sha256: digest.into(),
                programdata_account_sha256: String::new(),
                deployment_slot: 1,
                semantic_release_id: String::new(),
            }],
        }
    }

    /// THE LOOPBACK LIFECYCLE'S CORE IS THIS TREE'S CORE. It reaches here
    /// labelled `observed-programdata-account` because the profile installs the
    /// checked candidate into genesis and then reads it back, and the bytes the
    /// chain runs are the exact candidate the plan carries.
    #[test]
    fn a_core_this_run_installed_into_its_own_genesis_projects_recorded() {
        let digest = "22".repeat(32);
        let pin = core_pin(&digest, OBSERVED_DEPLOYMENT_SOURCE_V1);
        let set = local_mutable_set(&pin.program_id, &digest);
        assert_eq!(
            core_product_graph_projection_v1(&pin, Some(&set))
                .expect("the candidate this run installed is this tree's Core"),
            CoreProductGraphProjectionV1::Recorded,
        );
    }

    /// AND THE EXEMPTION IS THE LOCAL-MUTABLE PROFILE'S ALONE. The same pin
    /// with no local-mutable set -- which is every devnet plan -- still
    /// refuses, so nothing about a deployed cohort got easier.
    #[test]
    fn the_same_core_without_a_local_mutable_set_still_refuses() {
        let digest = "22".repeat(32);
        let pin = core_pin(&digest, OBSERVED_DEPLOYMENT_SOURCE_V1);
        core_product_graph_projection_v1(&pin, None)
            .expect_err("a deployed Core with no statement and no local install must refuse");
    }

    /// A LOCAL-MUTABLE SET DESCRIBING DIFFERENT BYTES DOES NOT EXEMPT THEM.
    /// The row has to be the Core the plan pins, at the same program id, with
    /// its live ProgramData hashing to that same candidate.
    #[test]
    fn a_local_mutable_set_naming_other_bytes_does_not_exempt_the_pin() {
        let digest = "22".repeat(32);
        let other = "33".repeat(32);
        let pin = core_pin(&digest, OBSERVED_DEPLOYMENT_SOURCE_V1);
        let set = local_mutable_set(&pin.program_id, &other);
        core_product_graph_projection_v1(&pin, Some(&set))
            .expect_err("a set describing other bytes is not a statement about these");
    }

    /// AN UNKNOWN DEPLOYED CORE IS A REFUSAL, NOT A GUESS -- and it names the
    /// digest, the program, and the one command that settles it.
    #[test]
    fn an_observed_core_this_driver_cannot_model_refuses_before_any_transaction() {
        let unknown = "11".repeat(32);
        let error = core_product_graph_projection_v1(
            &core_pin(&unknown, OBSERVED_DEPLOYMENT_SOURCE_V1),
            None,
        )
        .expect_err("a deployed Core with no recorded statement must refuse");
        let text = error.to_string();
        assert!(text.contains(&unknown), "{text}");
        assert!(
            text.contains("9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB"),
            "{text}"
        );
        assert!(text.contains("b312ce3c4"), "{text}");
    }

    fn observed_core_row(live_elf_sha256: Option<&str>) -> ObservedRoleV1 {
        ObservedRoleV1 {
            role: CORE_ROLE_NAME_V1.into(),
            program_id: "9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB".into(),
            programdata_id: "CC39Q4RstSZBniSZZASYoZXMyQtTari3WHZs9Zscgt2t".into(),
            observed_slot: Some(492_226_262),
            pinned_slot: 492_226_262,
            observed_authority: None,
            pinned_authority: None,
            observed_owner: None,
            observed_executable: Some(false),
            observed_live_elf_sha256: live_elf_sha256.map(str::to_owned),
            pinned_live_elf_sha256: String::new(),
            checked_candidate_elf_sha256: String::new(),
            live_elf_padding_bytes: 0,
            observed_data_len: None,
        }
    }

    /// The preflight is a reading, not a document check: the projection is
    /// admitted only for a plan whose Core is the one the cluster is running.
    #[test]
    fn a_cluster_running_a_different_core_refuses_the_projection() {
        let mut pin = core_pin(COHORT_14_CORE_ELF_SHA256, OBSERVED_DEPLOYMENT_SOURCE_V1);
        pin.live_elf_sha256 = "aa".repeat(32);
        assert_eq!(
            authenticate_core_bump_projection_v1(
                &pin,
                &[observed_core_row(Some(&pin.live_elf_sha256))],
                None
            )
            .expect("the plan's Core is the live Core"),
            CoreProductGraphProjectionV1::Unrecorded,
        );

        let error = authenticate_core_bump_projection_v1(
            &pin,
            &[observed_core_row(Some(&"bb".repeat(32)))],
            None,
        )
        .expect_err("a live Core the plan does not describe cannot be modelled");
        let text = error.to_string();
        assert!(text.contains(&"aa".repeat(32)), "{text}");
        assert!(text.contains(&"bb".repeat(32)), "{text}");
    }

    /// A substrate reading with no Core row cannot support a projection, and
    /// an absent row is not a passing one.
    #[test]
    fn a_substrate_reading_without_a_core_row_refuses() {
        let pin = core_pin(COHORT_14_CORE_ELF_SHA256, OBSERVED_DEPLOYMENT_SOURCE_V1);
        let mut trading = observed_core_row(None);
        trading.role = "trading".into();
        assert!(authenticate_core_bump_projection_v1(&pin, &[trading], None).is_err());
        assert!(authenticate_core_bump_projection_v1(&pin, &[], None).is_err());
    }

    /// The two lists must stay disjoint. One digest cannot both record and not
    /// record, and a duplicated row is how a list acquires that claim.
    #[test]
    fn no_core_digest_appears_in_both_lists() {
        // BOTH lists, not just the unrecorded one. Iterating one of them made
        // the shape assertions below apply to whichever list happened to be
        // non-empty, so the first row ever written to the other one would have
        // been unchecked -- and it is the recorded list that grows every time a
        // cohort is deployed.
        for digest in UNRECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1
            .iter()
            .chain(RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1.iter())
            .copied()
        {
            assert!(
                !(UNRECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1.contains(&digest)
                    && RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1.contains(&digest)),
                "{digest} claims both projections"
            );
            assert_eq!(digest.len(), 64, "{digest} is not a sha256");
            assert!(
                digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
                "{digest} is not lowercase hex"
            );
        }
    }
}
