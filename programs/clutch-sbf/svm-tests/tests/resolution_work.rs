//! Promotion gate for the unreachable ResolutionWork draft.
//!
//! This is intentionally not real-bank success evidence yet. It lives in the
//! real-SVM workspace so integration must replace these STOP assertions with
//! the pinned Begin/Fold/Finalize/Abort campaign at the same path instead of
//! accidentally leaving a host-only test behind.

const SEMANTIC_DRAFT: &str = include_str!("../../program/src/instructions/resolution_work.rs");
const INSTRUCTION_EXPORTS: &str = include_str!("../../program/src/instructions/mod.rs");
const ROUTER: &str = include_str!("../../program/src/dispatch.rs");
const IMPLEMENTATION_NOTE: &str =
    include_str!("../../../../docs/implementation/RESOLUTION_WORK_SBF.md");

#[test]
fn proposed_runtime_is_explicitly_unreachable_until_real_bank_replaces_this_gate() {
    assert!(SEMANTIC_DRAFT.contains("**STOP:**"));
    assert!(SEMANTIC_DRAFT.contains("temporary path import"));
    assert!(!INSTRUCTION_EXPORTS.contains("pub mod resolution_work;"));
    assert!(!ROUTER.contains("ResolutionWork"));
    assert!(IMPLEMENTATION_NOTE.contains("No CU, rent, account-count admission"));
    assert!(IMPLEMENTATION_NOTE.contains("real `cargo\nbuild-sbf` artifact"));
}

#[test]
fn required_real_bank_campaign_is_pinned_before_promotion() {
    for requirement in [
        "Begin at the exact rent minimum",
        "Fold sizes 1 through 4",
        "Finalize before end, at end, and late after expiry",
        "payer-only unstarted Abort",
        "alternate same-domain archive account",
        "terminal replay after account close",
        "injected late Finalize/Abort failure",
        "equivalence with monolithic v4",
        "ELF SHA-256",
    ] {
        assert!(
            IMPLEMENTATION_NOTE.contains(requirement),
            "missing promotion obligation: {requirement}"
        );
    }
}
