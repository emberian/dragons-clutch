//! The step table and the honest boundary enumeration.
//!
//! Both campaign profiles run the same real public prefix — collateral mint,
//! sealed policy/grid/Terms artifacts, Realm, Profile, and `CreateMarket` —
//! and then assert refusal boundaries.  The funded mock walk of
//! `joined_lifecycle.rs` does NOT survive on a public cluster; the exact list
//! of steps that die, and why, is [`devnet_impossible`].  A shorter honest
//! walk beats a faked one.

use clutch_sbf::error::ClutchError;
use clutch_solana_layout::{account_len, artifact::ARTIFACT_CHUNK_BYTES};

/// Which deployed ELF the campaign expects at `--program-id`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    /// The sealed default empty-registry ELF: the fail-closed campaign.
    Default,
    /// The explicitly NON-PRODUCTION mock-source ELF.
    Mock,
}

impl Profile {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "default" => Some(Self::Default),
            "mock" => Some(Self::Mock),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Mock => "mock",
        }
    }
}

/// What one step must do on-chain to count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepKind {
    /// The transaction confirms with no error and its reloads match.
    Accept,
    /// The transaction confirms WITH exactly `Custom(code)` at instruction
    /// index 1 (after the compute-budget instruction), and every watched
    /// account is byte-identical before and after.
    Refuse { code: u32 },
}

/// One planned campaign step, by stable name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepSpec {
    pub name: String,
    pub kind: StepKind,
}

fn accept(name: impl Into<String>) -> StepSpec {
    StepSpec {
        name: name.into(),
        kind: StepKind::Accept,
    }
}

fn refuse(name: impl Into<String>, code: ClutchError) -> StepSpec {
    StepSpec {
        name: name.into(),
        kind: StepKind::Refuse { code: code as u32 },
    }
}

fn artifact_steps(out: &mut Vec<StepSpec>, route: &str, body_len: usize) {
    out.push(accept(format!("{route}-artifact-begin")));
    let mut cursor = 0;
    while cursor < body_len {
        out.push(accept(format!("{route}-artifact-write-{cursor}")));
        cursor += ARTIFACT_CHUNK_BYTES.min(body_len - cursor);
    }
    out.push(accept(format!("{route}-artifact-seal")));
}

/// The exact byte length of the sealed collateral-policy artifact.
pub const POLICY_BODY_BYTES: usize = 266;

/// The full ordered step table of one campaign profile.
///
/// The two profiles share an identical accepted public prefix; they differ in
/// exactly one expectation: the code with which the deployed ELF refuses the
/// public `InitSourceSpec` route.
///
/// * `default` refuses `0x0079` (`SourceReleaseUnavailable`): the compiled
///   registry is empty, the refusal fires before authentication.
/// * `mock` refuses `0x007a` (`SourceAdmissionFailed`): the walk spec IS the
///   registered release, so the refusal falls through to the deployment
///   authenticator — which cannot be satisfied on a public cluster because
///   the laboratory provider trio is unconstructible there.
pub fn step_table(profile: Profile) -> Vec<StepSpec> {
    let mut steps = vec![
        accept("fund-actor"),
        accept("create-collateral-mint"),
        accept("create-actor-collateral-token"),
        accept("create-bearer-collateral-token"),
        accept("mint-collateral-and-freeze"),
    ];
    artifact_steps(&mut steps, "policy", POLICY_BODY_BYTES);
    steps.push(accept("init-realm"));
    steps.push(accept("init-profile"));
    artifact_steps(&mut steps, "grid", account_len::PRICE_GRID);
    artifact_steps(&mut steps, "terms", account_len::TERMS);
    steps.push(accept("create-market"));

    let init_spec_code = match profile {
        Profile::Default => ClutchError::SourceReleaseUnavailable,
        Profile::Mock => ClutchError::SourceAdmissionFailed,
    };
    steps.push(refuse("init-source-spec-refused", init_spec_code));
    steps.push(refuse(
        "init-source-archive-refused",
        ClutchError::SourceAdmissionFailed,
    ));
    steps.push(refuse("endow-refused-no-spec", ClutchError::WrongProgramOwner));
    steps
}

/// One local-walk step (or injected prerequisite) that cannot run on a public
/// cluster, with the reason and what the campaign asserts instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Boundary {
    /// The joined-lifecycle step or injected prerequisite that dies.
    pub local_step: &'static str,
    /// Why a public cluster structurally cannot host it.
    pub reason: &'static str,
    /// The on-chain refusal (or record) this campaign asserts in its place.
    pub asserted_instead: &'static str,
}

/// The exact devnet-impossible enumeration for one profile.
///
/// Everything NOT listed here — the collateral mint, both token accounts, the
/// three artifact uploads, Realm, Profile, and the full `CreateMarket` market
/// plane — survives on devnet as real confirmed public transactions.
pub fn devnet_impossible(profile: Profile) -> Vec<Boundary> {
    match profile {
        Profile::Default => vec![Boundary {
            local_step: "inject-canonical-source-spec (default campaign's only injected prerequisite)",
            reason: "a program-owned 292-byte SourceSpec image can only be written by the \
                     program itself, and the sole instruction that writes one \
                     (InitSourceSpec) refuses 0x0079 on this ELF; no genesis injection \
                     exists on a public cluster",
            asserted_instead: "init-source-spec-refused asserts Custom(0x0079) at the public \
                               route — the same closed-registry gate the local walk sharpens \
                               onto Endow — and endow-refused-no-spec asserts Custom(0x0004) \
                               at the state-role gate, so the sharpened \
                               'Endow(injected canonical spec) refuses 0x0079' variant is \
                               recorded as devnet-impossible rather than faked",
        }],
        Profile::Mock => vec![
            Boundary {
                local_step: "inject-mock-provider-program (0xb2.., executable, owner 0xb3.., \
                             body MOCK-PROVIDER-V1)",
                reason: "triply impossible on a public cluster: (1) no private key is known \
                         for the fixed address [0xb2; 32], so no transaction can create any \
                         account there; (2) executable accounts are produced only by BPF \
                         loader deployment, which sets loader ownership and ELF bytes, never \
                         an arbitrary 16-byte body; (3) account data can be written only by \
                         its owner program and [0xb3; 32] is not a deployed program",
                asserted_instead: "init-source-spec-refused asserts Custom(0x007a): the \
                                   registered walk spec reaches the deployment authenticator, \
                                   which refuses the absent provider account \
                                   (ProviderProgramOwnerMismatch) before any state write",
            },
            Boundary {
                local_step: "inject-mock-deployment-evidence (0xd4.., owner 0xd5.., body \
                             DEP1+generation)",
                reason: "same fixed-address impossibility, and only the owner program \
                         [0xd5; 32] — which is not deployable — could write the DEP1 body",
                asserted_instead: "covered by the same Custom(0x007a) authenticator refusal",
            },
            Boundary {
                local_step: "inject-mock-source-record (0xc3.., owner 0xb2.., 77-byte SRC1 \
                             record, host-rewritten three times between appends)",
                reason: "same fixed-address impossibility; additionally the local walk \
                         rewrites this account from the host between appends, standing in \
                         for the provider program's own writes — there is no on-chain \
                         equivalent of a host write",
                asserted_instead: "covered by the same Custom(0x007a) authenticator refusal",
            },
            Boundary {
                local_step: "init-source-archive, append-source-archive x2, \
                             seal-source-archive (public authenticated source route)",
                reason: "the route verifies the SourceSpec account first, and that account \
                         can never exist on a public cluster because InitSourceSpec refuses \
                         (see above); appends additionally re-authenticate the \
                         unconstructible provider trio against the live Clock",
                asserted_instead: "init-source-archive-refused asserts Custom(0x007a) at the \
                                   spec-verification gate (absent spec is not program-owned)",
            },
            Boundary {
                local_step: "endow, split, materialize, bearer-transfer (funded segment)",
                reason: "Endow re-authenticates the registered SourceSpec at the collateral \
                         boundary; with no spec account the state-role gate refuses before \
                         any value moves, and every later step spends what Endow admits",
                asserted_instead: "endow-refused-no-spec asserts Custom(0x0004) \
                                   (WrongProgramOwner) with every watched account \
                                   byte-identical",
            },
            Boundary {
                local_step: "inject-evidence-buffer + resolve (native point resolution)",
                reason: "the caller-supplied evidence buffer must be program-owned and no \
                         public instruction constructs an arbitrary program-owned buffer; \
                         resolution further requires the sealed archive that cannot exist",
                asserted_instead: "recorded as devnet-impossible; resolution evidence remains \
                                   local SBF-executed evidence only",
            },
            Boundary {
                local_step: "redeem-internal x4, redeem-external, withdraw-cash x2 \
                             (value exits)",
                reason: "all are post-resolution value exits over a funded Hoard; both \
                         prerequisites are unreachable on a public cluster",
                asserted_instead: "recorded as devnet-impossible; the campaign instead proves \
                                   the deployed ELF refuses value admission exactly as sealed",
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_names_are_unique_and_ordered_identically_across_profiles() {
        for profile in [Profile::Default, Profile::Mock] {
            let steps = step_table(profile);
            let mut names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
            let total = names.len();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), total, "{profile:?} step names collide");
        }
        let default_names: Vec<String> = step_table(Profile::Default)
            .into_iter()
            .map(|step| step.name)
            .collect();
        let mock_names: Vec<String> = step_table(Profile::Mock)
            .into_iter()
            .map(|step| step.name)
            .collect();
        assert_eq!(default_names, mock_names);
    }

    #[test]
    fn the_profiles_differ_in_exactly_the_init_source_spec_code() {
        let default_steps = step_table(Profile::Default);
        let mock_steps = step_table(Profile::Mock);
        let mut differences = 0;
        for (a, b) in default_steps.iter().zip(mock_steps.iter()) {
            if a != b {
                differences += 1;
                assert_eq!(a.name, "init-source-spec-refused");
                assert_eq!(a.kind, StepKind::Refuse { code: 0x0079 });
                assert_eq!(b.kind, StepKind::Refuse { code: 0x007a });
            }
        }
        assert_eq!(differences, 1);
    }

    #[test]
    fn refusal_codes_are_the_sealed_error_values() {
        assert_eq!(ClutchError::SourceReleaseUnavailable as u32, 0x0079);
        assert_eq!(ClutchError::SourceAdmissionFailed as u32, 0x007a);
        assert_eq!(ClutchError::WrongProgramOwner as u32, 0x0004);
        let steps = step_table(Profile::Default);
        let refusals: Vec<u32> = steps
            .iter()
            .filter_map(|step| match step.kind {
                StepKind::Refuse { code } => Some(code),
                StepKind::Accept => None,
            })
            .collect();
        assert_eq!(refusals, [0x0079, 0x007a, 0x0004]);
    }

    #[test]
    fn accepted_prefix_ends_at_create_market_and_only_refusals_follow() {
        for profile in [Profile::Default, Profile::Mock] {
            let steps = step_table(profile);
            let boundary = steps
                .iter()
                .position(|step| step.name == "create-market")
                .expect("create-market is planned");
            for step in &steps[..=boundary] {
                assert_eq!(step.kind, StepKind::Accept, "{}", step.name);
            }
            assert_eq!(steps.len() - boundary - 1, 3);
            for step in &steps[boundary + 1..] {
                assert!(
                    matches!(step.kind, StepKind::Refuse { .. }),
                    "{}",
                    step.name
                );
            }
        }
    }

    #[test]
    fn every_boundary_names_a_reason_and_a_replacement() {
        for profile in [Profile::Default, Profile::Mock] {
            let boundaries = devnet_impossible(profile);
            assert!(!boundaries.is_empty());
            for boundary in &boundaries {
                assert!(!boundary.local_step.is_empty());
                assert!(boundary.reason.len() > 40, "{}", boundary.local_step);
                assert!(
                    !boundary.asserted_instead.is_empty(),
                    "{}",
                    boundary.local_step
                );
            }
        }
        /* The mock walk loses its entire funded segment; the enumeration must
         * say so step family by step family. */
        assert_eq!(devnet_impossible(Profile::Mock).len(), 7);
        assert_eq!(devnet_impossible(Profile::Default).len(), 1);
    }

    #[test]
    fn profile_parsing_is_exact() {
        assert_eq!(Profile::parse("default"), Some(Profile::Default));
        assert_eq!(Profile::parse("mock"), Some(Profile::Mock));
        assert_eq!(Profile::parse("Default"), None);
        assert_eq!(Profile::parse("prod"), None);
    }
}
