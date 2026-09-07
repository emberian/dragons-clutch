//! The host's one author for the funded deadline-failure walk: the exhausted
//! terminal as a driven act.
//!
//! `RelayActionV1::CommitDeadlineFailure` is a 32-byte instruction naming only
//! the Market generation and the terminal sequence. Which arm the walk takes is
//! read by the program off the Source's own phase --
//! `Primary → Exhausted → FailureCommitted` for a market that bought no ladder,
//! `Exhausted → FailureCommitted` for one whose funded ladder already walked
//! every leg it sold -- and the outcome it commits is the Product's OWN failure
//! selector out of the finalized result domain. So there is nothing here for a
//! host to decide. What there is, is a frame to assemble in exactly the order
//! the relay contract declares, a certificate seat to derive at the
//! `ResolutionFailure` kind, and an admissibility comparison to reproduce off
//! chain so a walk that would refuse refuses HERE, by name, before a lamport
//! moves.
//!
//! # Why the seat is the failure kind and never a choice
//!
//! The certificate kind is a PDA *seed*, so the failure a deadline walk writes
//! and the success an observation writes live at different addresses for one
//! Source state at one sequence. This module derives the seat from
//! [`ResolutionCertificateKindV2::ResolutionFailure`] and nothing else; a
//! caller cannot aim the walk at a success seat, and the program refuses a seat
//! that is not the one its own kind derives.
//!
//! # The two arms, and the one the crank owns
//!
//! A market standing on `Recovery` is not this route's to finish: its current
//! rung still has time on it, or it does not and the permissionless crank
//! (`advance-recovery`) is what moves it -- onto the next funded rung, or to
//! `Exhausted`. A market standing on `Primary` WITH a policy is refused for the
//! same reason: `exhaust_after_primary_deadline` refuses a recovery-bearing
//! material by name because skipping paid-for legs would take an outcome away
//! from the holders who paid for them (decision 0027). Both refusals are
//! reproduced here so the sentence a host reads names the crank it should have
//! run instead of a `Transition` code.
//!
//! Three consumers call this and none restates it: the successor's
//! `commit-deadline-failure` driver, the journey tier's `failure` walk, and the
//! ladder tier's exhausted path. A fourth spelling of the twenty-two positions
//! would eventually disagree with the other three.

use core::fmt;

use dclutch_product::ResultDomainV2;
use dclutch_source::relay::{
    frame::{
        RelayAccountNameV1, RelayAccountPrivilegeV1, RelayFrameKindV1, relay_frame_roles_v1,
        validate_relay_frame_v1,
    },
    instruction::CommitDeadlineFailureInstructionV1,
};
use dclutch_source::resolution::{
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, ResolutionCertificateKindV2,
};
use dclutch_source::{SourceResolutionPhaseV1, WindowSpecV1};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// The exact account count the route demands, restated from the relay frame so
/// a frame that drifts fails here rather than after a cluster round trip.
pub const COMMIT_DEADLINE_FAILURE_FRAME_ACCOUNTS_V1: usize = 22;

/// One finalized Registry record's two coordinates: the raw record and the
/// staging vacancy that proves it immutable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedPairV1 {
    /// The raw immutable record.
    pub raw: Pubkey,
    /// The finalized staging vacancy.
    pub staging: Pubkey,
}

/// Everything the twenty-two positions are filled from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeadlineFailureCoordinatesV1 {
    /// The permissionless worker who signs, pays, and is PAID the bounty.
    pub worker: Pubkey,
    /// The Core Market.
    pub market: Pubkey,
    /// The Core program that owns the Market.
    pub core_program: Pubkey,
    /// The Registry-owned activation cache for the Market's release set.
    pub activation: Pubkey,
    /// The Resolution program this walk is addressed to.
    pub resolution_program: Pubkey,
    /// The Resolution-owned `SourceResolutionStateV2`.
    pub source_state: Pubkey,
    /// The raw immutable `SourceMaterialV3` record and its vacancy.
    pub material: FinalizedPairV1,
    /// The raw immutable `WindowSpecV1` record and its vacancy.
    pub window: FinalizedPairV1,
    /// The Product Runtime V2 Product record and its vacancy.
    pub product: FinalizedPairV1,
    /// The Product Runtime V2 result domain and its vacancy.
    pub result_domain: FinalizedPairV1,
    /// The Product Runtime V2 portfolio record and its vacancy.
    pub portfolio: FinalizedPairV1,
    /// The `CapabilityManifestV1` the funding compartments quote, and its vacancy.
    pub manifest: FinalizedPairV1,
    /// The Resolution-owned three-compartment funding ledger the walk debits.
    pub funding_ledger: Pubkey,
}

/// Why a deadline failure cannot be built or is not admissible yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineFailureErrorV1 {
    /// The market stands on `Primary` and its material bought a ladder: the
    /// failure walk may not skip legs the holders paid for. Crank the ladder.
    LadderNotWalked,
    /// The market stands on a funded rung. Either the rung still has time on
    /// it, or the crank -- not this walk -- is what ends it.
    LadderStanding {
        /// The rung the market stands on.
        active_attempt: u8,
    },
    /// The market has already reached a terminal, or is retired.
    AlreadyTerminal(SourceResolutionPhaseV1),
    /// The primary window's deadline has not passed: a walk is admissible
    /// STRICTLY after `window.end + max_age`.
    DeadlineNotReached {
        /// The last second an honest observation may still land.
        due_unix_seconds: i64,
        /// What the chain's clock read.
        observed_unix_seconds: i64,
    },
    /// The primary window's `end + max_age` overflowed an `i64`.
    Arithmetic,
    /// The relay contract declares a frame width this module does not fill.
    FrameWidth {
        /// What the contract declares.
        declared: usize,
    },
    /// A filled position's role name is not the contract's at that index.
    FrameOrder {
        /// The position that disagreed.
        index: usize,
    },
    /// The frame's privileges or no-alias policy refused offline.
    FramePrivileges,
    /// The instruction wire refused (a zero terminal sequence, for one).
    Wire,
    /// The result domain would not say how wide it is.
    ResultDomain,
}

impl fmt::Display for DeadlineFailureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LadderNotWalked => formatter.write_str(
                "the market stands on its primary leg and its material bought a funded ladder: \
                 the failure walk may not skip legs the holders paid for \
                 (exhaust_after_primary_deadline refuses a recovery-bearing material by name). \
                 Crank the ladder with advance-recovery until the Source reads Exhausted, then \
                 walk it",
            ),
            Self::LadderStanding { active_attempt } => write!(
                formatter,
                "the market stands on funded rung {active_attempt}: either that rung still has \
                 time on it or the permissionless crank is what ends it. This walk commits the \
                 failure selector only from Primary (no ladder) or Exhausted (a walked ladder)"
            ),
            Self::AlreadyTerminal(phase) => write!(
                formatter,
                "the market stands at {phase:?}, which is already an end: a deadline walk commits \
                 the failure selector only from Primary or Exhausted"
            ),
            Self::DeadlineNotReached {
                due_unix_seconds,
                observed_unix_seconds,
            } => write!(
                formatter,
                "the primary leg is due at {due_unix_seconds} and the chain clock reads \
                 {observed_unix_seconds}: a deadline walk is admissible STRICTLY after the \
                 deadline, because the last second an honest observation may land and the first \
                 second a walk may run are different seconds"
            ),
            Self::Arithmetic => formatter.write_str("primary window end + max_age overflows"),
            Self::FrameWidth { declared } => write!(
                formatter,
                "the CommitDeadlineFailure frame declares {declared} positions and this builder \
                 fills {COMMIT_DEADLINE_FAILURE_FRAME_ACCOUNTS_V1}"
            ),
            Self::FrameOrder { index } => write!(
                formatter,
                "position {index} of the CommitDeadlineFailure frame is not the role this builder \
                 filled it with"
            ),
            Self::FramePrivileges => formatter.write_str(
                "the CommitDeadlineFailure frame's privileges or no-alias policy refused offline",
            ),
            Self::Wire => formatter.write_str(
                "the CommitDeadlineFailure request refused to encode; the wire refuses a zero \
                 terminal sequence",
            ),
            Self::ResultDomain => {
                formatter.write_str("the finalized result domain would not state its outcome count")
            }
        }
    }
}

/// Which arm of the walk the market's own state selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineFailureArmV1 {
    /// No ladder: the walk spends the primary deadline itself,
    /// `Primary → Exhausted → FailureCommitted`.
    PrimaryDeadline,
    /// A walked ladder: the last crank left the market `Exhausted`, and the
    /// walk commits `Exhausted → FailureCommitted`.
    LadderExhausted,
}

impl DeadlineFailureArmV1 {
    /// The driver's own word for the arm, for a label and an evidence field.
    pub const fn label(self) -> &'static str {
        match self {
            Self::PrimaryDeadline => "primary-deadline",
            Self::LadderExhausted => "ladder-exhausted",
        }
    }
}

/// The whole of what a host decides, and it decides none of it freely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeadlineFailureDecisionV1 {
    /// Which arm the Source's phase selects.
    pub arm: DeadlineFailureArmV1,
    /// The last second an honest observation may still land, on the
    /// primary-deadline arm. `None` on the exhausted arm: the last crank
    /// already spent the final rung's own deadline.
    pub due_unix_seconds: Option<i64>,
    /// The selector the walk will commit: the Product's own failure region,
    /// read off the finalized result domain rather than typed.
    pub failure_selector: u32,
    /// The Product's outcome count, so a reader can say which cell that is.
    pub outcome_count: u32,
}

impl DeadlineFailureDecisionV1 {
    /// Whether the walk is admissible at `observed_unix_seconds`, by name.
    ///
    /// Reproduces `SourceResolutionStateV2::exhaust_after_primary_deadline`'s
    /// `current_unix_seconds <= due` refusal off chain. On the exhausted arm
    /// the walk is admissible at once.
    pub fn admissible_at(self, observed_unix_seconds: i64) -> Result<(), DeadlineFailureErrorV1> {
        match self.due_unix_seconds {
            Some(due_unix_seconds) if observed_unix_seconds <= due_unix_seconds => {
                Err(DeadlineFailureErrorV1::DeadlineNotReached {
                    due_unix_seconds,
                    observed_unix_seconds,
                })
            }
            _ => Ok(()),
        }
    }

    /// The first second the walk becomes admissible, for a bounded wait.
    pub fn first_admissible_second(self) -> Result<Option<i64>, DeadlineFailureErrorV1> {
        self.due_unix_seconds
            .map(|due| due.checked_add(1).ok_or(DeadlineFailureErrorV1::Arithmetic))
            .transpose()
    }
}

/// Decide which arm the walk takes, from facts the chain itself holds.
///
/// `has_policy` is whether the material selects a `RecoveryPolicyV2` -- the
/// founding's evidence names the record or leaves the row out, and the program
/// reads the same fact off `SourceMaterialV3::recovery_policy()`. NEITHER the
/// arm nor the selector is a choice: the arm is the phase's, and the selector
/// is `ResultDomainV2::failure_selector`, the same function
/// `plan_deadline_failure_v1` compares the kernel's decision against.
pub fn deadline_failure_decision_v1(
    phase: SourceResolutionPhaseV1,
    active_attempt: u8,
    has_policy: bool,
    window: WindowSpecV1,
    result_domain: &ResultDomainV2<'_>,
) -> Result<DeadlineFailureDecisionV1, DeadlineFailureErrorV1> {
    let outcome_count = result_domain
        .outcome_count()
        .map_err(|_| DeadlineFailureErrorV1::ResultDomain)?;
    let failure_selector = result_domain.failure_selector();
    match phase {
        SourceResolutionPhaseV1::Primary if has_policy => {
            Err(DeadlineFailureErrorV1::LadderNotWalked)
        }
        SourceResolutionPhaseV1::Primary => Ok(DeadlineFailureDecisionV1 {
            arm: DeadlineFailureArmV1::PrimaryDeadline,
            due_unix_seconds: Some(
                window
                    .end_unix_seconds()
                    .checked_add(i64::from(window.max_age_seconds()))
                    .ok_or(DeadlineFailureErrorV1::Arithmetic)?,
            ),
            failure_selector,
            outcome_count,
        }),
        SourceResolutionPhaseV1::Recovery => {
            Err(DeadlineFailureErrorV1::LadderStanding { active_attempt })
        }
        SourceResolutionPhaseV1::Exhausted => Ok(DeadlineFailureDecisionV1 {
            arm: DeadlineFailureArmV1::LadderExhausted,
            due_unix_seconds: None,
            failure_selector,
            outcome_count,
        }),
        other => Err(DeadlineFailureErrorV1::AlreadyTerminal(other)),
    }
}

/// The certificate seat a deadline walk writes: the `ResolutionFailure` kind
/// seed under the Source state at the terminal sequence.
///
/// Read from the codec rather than written again here, because a second copy
/// of the seed would be a second author of where a certificate lives.
pub fn deadline_failure_certificate_v1(
    resolution_program: Pubkey,
    source_state: Pubkey,
    terminal_sequence: u64,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &[ResolutionCertificateKindV2::ResolutionFailure.kind_seed()],
            &terminal_sequence.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0
}

/// The frame's twenty-two keys, each carrying the role name it claims to be.
///
/// One author for the order, so the check against `relay_frame_roles_v1` and
/// the metas that are actually sent cannot drift apart -- and so a test can ask
/// the same question without a chain.
pub fn deadline_failure_keys_v1(
    coordinates: &DeadlineFailureCoordinatesV1,
    certificate: Pubkey,
) -> [(RelayAccountNameV1, Pubkey); COMMIT_DEADLINE_FAILURE_FRAME_ACCOUNTS_V1] {
    [
        (RelayAccountNameV1::Worker, coordinates.worker),
        (RelayAccountNameV1::Market, coordinates.market),
        (RelayAccountNameV1::CoreProgram, coordinates.core_program),
        (
            RelayAccountNameV1::RegistryActivation,
            coordinates.activation,
        ),
        (
            RelayAccountNameV1::SourceResolutionState,
            coordinates.source_state,
        ),
        (RelayAccountNameV1::ResolutionCertificate, certificate),
        (RelayAccountNameV1::SourceMaterial, coordinates.material.raw),
        (
            RelayAccountNameV1::SourceMaterialStagingVacancy,
            coordinates.material.staging,
        ),
        (RelayAccountNameV1::WindowSpec, coordinates.window.raw),
        (
            RelayAccountNameV1::WindowSpecStagingVacancy,
            coordinates.window.staging,
        ),
        (RelayAccountNameV1::ProductRecord, coordinates.product.raw),
        (
            RelayAccountNameV1::ProductRecordStagingVacancy,
            coordinates.product.staging,
        ),
        (
            RelayAccountNameV1::ResultDomain,
            coordinates.result_domain.raw,
        ),
        (
            RelayAccountNameV1::ResultDomainStagingVacancy,
            coordinates.result_domain.staging,
        ),
        (
            RelayAccountNameV1::PortfolioRecord,
            coordinates.portfolio.raw,
        ),
        (
            RelayAccountNameV1::PortfolioRecordStagingVacancy,
            coordinates.portfolio.staging,
        ),
        (
            RelayAccountNameV1::CapabilityManifest,
            coordinates.manifest.raw,
        ),
        (
            RelayAccountNameV1::CapabilityManifestStagingVacancy,
            coordinates.manifest.staging,
        ),
        (
            RelayAccountNameV1::ResolutionFunding,
            coordinates.funding_ledger,
        ),
        (RelayAccountNameV1::ClockSysvar, sysvar::clock::ID),
        (RelayAccountNameV1::RentSysvar, sysvar::rent::ID),
        (RelayAccountNameV1::SystemProgram, system_program::ID),
    ]
}

/// One planned deadline walk: the instruction and the seat it writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitDeadlineFailurePlanV1 {
    /// The exact instruction, over the relay contract's own ordered frame.
    pub instruction: Instruction,
    /// The `ResolutionFailure` seat this walk mints into.
    pub certificate: Pubkey,
    /// How many accounts the frame carries, for a transcript to count.
    pub frame_accounts: usize,
}

/// Build the one instruction a deadline walk sends.
///
/// THE FRAME, DRIVEN BY THE RELAY CONTRACT'S OWN TABLE. Signer and writable
/// come from `relay_frame_roles_v1`, never from this file, and
/// `validate_relay_frame_v1` then checks the count, every privilege and the
/// complete no-alias policy offline -- so a frame that would refuse as
/// `InvalidAccountFrame` refuses here instead. Every position is also checked
/// against the frame's own role NAME at that index, because privileges and the
/// no-alias rule alone would pass twenty-two keys in the wrong order.
pub fn build_commit_deadline_failure_v1(
    coordinates: &DeadlineFailureCoordinatesV1,
    generation: u64,
    terminal_sequence: u64,
) -> Result<CommitDeadlineFailurePlanV1, DeadlineFailureErrorV1> {
    let certificate = deadline_failure_certificate_v1(
        coordinates.resolution_program,
        coordinates.source_state,
        terminal_sequence,
    );
    let filled = deadline_failure_keys_v1(coordinates, certificate);
    let roles = relay_frame_roles_v1(RelayFrameKindV1::CommitDeadlineFailure);
    if roles.len() != COMMIT_DEADLINE_FAILURE_FRAME_ACCOUNTS_V1 {
        return Err(DeadlineFailureErrorV1::FrameWidth {
            declared: roles.len(),
        });
    }
    for (index, (role, (name, _))) in roles.iter().zip(filled.iter()).enumerate() {
        if role.name() != *name {
            return Err(DeadlineFailureErrorV1::FrameOrder { index });
        }
    }
    let privileges: Vec<RelayAccountPrivilegeV1> = roles
        .iter()
        .zip(filled.iter())
        .map(|(role, (_, key))| RelayAccountPrivilegeV1 {
            key: key.to_bytes(),
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        })
        .collect();
    validate_relay_frame_v1(RelayFrameKindV1::CommitDeadlineFailure, &privileges)
        .map_err(|_| DeadlineFailureErrorV1::FramePrivileges)?;
    let accounts: Vec<AccountMeta> = roles
        .iter()
        .zip(filled.iter())
        .map(|(role, (_, key))| AccountMeta {
            pubkey: *key,
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        })
        .collect();
    let data = CommitDeadlineFailureInstructionV1::new(generation, terminal_sequence)
        .map_err(|_| DeadlineFailureErrorV1::Wire)?
        .to_bytes()
        .map_err(|_| DeadlineFailureErrorV1::Wire)?
        .to_vec();
    Ok(CommitDeadlineFailurePlanV1 {
        frame_accounts: accounts.len(),
        instruction: Instruction {
            program_id: coordinates.resolution_program,
            accounts,
            data,
        },
        certificate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_source::{ContentId, WindowKind};

    fn pair(seed: u8) -> FinalizedPairV1 {
        FinalizedPairV1 {
            raw: Pubkey::new_from_array([seed; 32]),
            staging: Pubkey::new_from_array([seed.wrapping_add(1); 32]),
        }
    }

    fn coordinates() -> DeadlineFailureCoordinatesV1 {
        DeadlineFailureCoordinatesV1 {
            worker: Pubkey::new_from_array([0x01; 32]),
            market: Pubkey::new_from_array([0x02; 32]),
            core_program: Pubkey::new_from_array([0x03; 32]),
            activation: Pubkey::new_from_array([0x04; 32]),
            resolution_program: Pubkey::new_from_array([0x05; 32]),
            source_state: Pubkey::new_from_array([0x06; 32]),
            material: pair(0x10),
            window: pair(0x20),
            product: pair(0x30),
            result_domain: pair(0x40),
            portfolio: pair(0x50),
            manifest: pair(0x60),
            funding_ledger: Pubkey::new_from_array([0x70; 32]),
        }
    }

    fn window() -> WindowSpecV1 {
        let id = |tag: u8| ContentId::new([tag; 32]).expect("nonzero");
        WindowSpecV1::new(
            id(0x07),
            WindowKind::Terminal,
            1_000,
            2_000,
            300,
            1,
            id(0x06),
        )
        .expect("terminal window")
    }

    /// The twenty-two positions, their privileges and the no-alias rule are
    /// the relay contract's, and this asserts that this builder reads them
    /// from there rather than restating them.
    #[test]
    fn the_frame_is_the_relay_contracts_and_this_builder_only_fills_it() {
        let roles = relay_frame_roles_v1(RelayFrameKindV1::CommitDeadlineFailure);
        assert_eq!(roles.len(), COMMIT_DEADLINE_FAILURE_FRAME_ACCOUNTS_V1);
        // Position zero is the only signer and one of exactly four writable
        // positions: the worker it pays, the Source that becomes terminal, the
        // certificate this walk creates, and the ledger it spends.
        assert!(roles[0].is_signer() && roles[0].is_writable());
        let writable: Vec<usize> = roles
            .iter()
            .enumerate()
            .filter(|(_, role)| role.is_writable())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(writable, vec![0, 4, 5, 18]);
        assert_eq!(
            roles.iter().filter(|role| role.is_signer()).count(),
            1,
            "a permissionless walk has exactly one signer, and it is the worker it pays"
        );
    }

    /// Every key this builder fills is checked against the role the contract
    /// declares for that position, so an ordering slip cannot hide behind
    /// privileges that happen to match.
    #[test]
    fn every_filled_position_is_the_role_the_contract_declares() {
        let coordinates = coordinates();
        let filled = deadline_failure_keys_v1(&coordinates, Pubkey::new_from_array([0x08; 32]));
        let roles = relay_frame_roles_v1(RelayFrameKindV1::CommitDeadlineFailure);
        assert_eq!(roles.len(), filled.len());
        for (index, (role, (name, _))) in roles.iter().zip(filled.iter()).enumerate() {
            assert_eq!(role.name(), *name, "position {index}");
        }
        // The Product graph the crank frame does not carry, named rather than
        // counted: a walk commits an outcome, so it needs the domain that says
        // which cell is the failure cell.
        assert_eq!(filled[12].0, RelayAccountNameV1::ResultDomain);
        assert_eq!(filled[18].0, RelayAccountNameV1::ResolutionFunding);
    }

    /// The built instruction carries the contract's privileges and the failure
    /// seat, and the seat is not the success seat, nor either crank seat.
    #[test]
    fn the_plan_writes_the_failure_seat_and_no_other_kinds() {
        let coordinates = coordinates();
        let plan =
            build_commit_deadline_failure_v1(&coordinates, 7, 1).expect("a well-formed frame");
        assert_eq!(
            plan.frame_accounts,
            COMMIT_DEADLINE_FAILURE_FRAME_ACCOUNTS_V1
        );
        assert_eq!(plan.instruction.program_id, coordinates.resolution_program);
        assert_eq!(plan.instruction.accounts[5].pubkey, plan.certificate);
        assert!(plan.instruction.accounts[5].is_writable);
        for kind in [
            ResolutionCertificateKindV2::ResolutionSuccess,
            ResolutionCertificateKindV2::RecoveryAdvanced,
            ResolutionCertificateKindV2::Exhausted,
        ] {
            let other = Pubkey::find_program_address(
                &[
                    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                    coordinates.source_state.as_ref(),
                    &[kind.kind_seed()],
                    &1_u64.to_le_bytes(),
                ],
                &coordinates.resolution_program,
            )
            .0;
            assert_ne!(plan.certificate, other, "{kind:?} is a different seat");
        }
        // The wire refuses a zero terminal sequence, so the builder does too
        // rather than building a request that cannot encode.
        assert_eq!(
            build_commit_deadline_failure_v1(&coordinates, 7, 0),
            Err(DeadlineFailureErrorV1::Wire)
        );
    }

    /// Two aliased positions refuse offline, before a cluster is asked.
    #[test]
    fn an_aliased_frame_refuses_offline_by_name() {
        let mut coordinates = coordinates();
        coordinates.funding_ledger = coordinates.market;
        assert_eq!(
            build_commit_deadline_failure_v1(&coordinates, 7, 1),
            Err(DeadlineFailureErrorV1::FramePrivileges)
        );
    }

    /// Both arms, the two phases neither owns, and the ends.
    ///
    /// The whole decision a host makes, checked with no chain: which arm the
    /// phase selects, which second the walk becomes admissible, and which
    /// selector it will commit -- read off the result domain, never typed.
    /// A four-cell result domain -- two interior cuts, the two tails and the
    /// explicit failure outcome -- compiled through the Product's own encoder
    /// so the selector the decision reads is the one a founding would write.
    fn four_cell_domain() -> Vec<u8> {
        use dclutch_product::{
            ContentId as ProductId, ResultDomainInputV2, compile_result_domain_v2,
            result_domain_record_bytes,
        };
        let id = |tag: u8| ProductId::new([tag; 32]).expect("nonzero");
        let cuts: [i128; 2] = [14_800, 15_200];
        let mut bytes = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("width")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id: id(0xa1),
                coordinate_domain_id: id(0xa2),
                result_unit_id: id(0xa3),
                liability_basis_id: id(0xa4),
                representation_release_id: id(0xa5),
                mapping_release_id: id(0xa6),
                cut_denominator: 100,
                cuts: &cuts,
            },
            &mut bytes,
        )
        .expect("a four-cell result domain");
        bytes
    }

    #[test]
    fn the_decision_names_the_arm_the_second_and_the_selector() {
        let domain_bytes = four_cell_domain();
        let domain = ResultDomainV2::decode(&domain_bytes).expect("a four-cell result domain");
        let window = window();

        // NO LADDER, PRIMARY. The walk spends the deadline itself and is
        // admissible one second after the window's closing plus its liveness
        // grace -- 2,000 + 300.
        let primary = deadline_failure_decision_v1(
            SourceResolutionPhaseV1::Primary,
            0,
            false,
            window,
            &domain,
        )
        .expect("primary decision");
        assert_eq!(primary.arm, DeadlineFailureArmV1::PrimaryDeadline);
        assert_eq!(primary.due_unix_seconds, Some(2_300));
        assert_eq!(
            primary.failure_selector, 3,
            "the last cell is the failure cell"
        );
        assert_eq!(primary.outcome_count, 4);
        assert_eq!(
            primary.admissible_at(2_300),
            Err(DeadlineFailureErrorV1::DeadlineNotReached {
                due_unix_seconds: 2_300,
                observed_unix_seconds: 2_300,
            }),
            "standing exactly on the deadline is still the honest observer's second"
        );
        assert_eq!(primary.admissible_at(2_301), Ok(()));
        assert_eq!(primary.first_admissible_second(), Ok(Some(2_301)));

        // A LADDER, PRIMARY. Not this route's: the crank owns the legs.
        assert_eq!(
            deadline_failure_decision_v1(
                SourceResolutionPhaseV1::Primary,
                0,
                true,
                window,
                &domain
            ),
            Err(DeadlineFailureErrorV1::LadderNotWalked)
        );
        // A RUNG. Still the crank's.
        assert_eq!(
            deadline_failure_decision_v1(
                SourceResolutionPhaseV1::Recovery,
                1,
                true,
                window,
                &domain
            ),
            Err(DeadlineFailureErrorV1::LadderStanding { active_attempt: 1 })
        );
        // EXHAUSTED. The walk's own arm, admissible at once: the last crank
        // already spent the final rung's committed deadline.
        let exhausted = deadline_failure_decision_v1(
            SourceResolutionPhaseV1::Exhausted,
            0,
            true,
            window,
            &domain,
        )
        .expect("exhausted decision");
        assert_eq!(exhausted.arm, DeadlineFailureArmV1::LadderExhausted);
        assert_eq!(exhausted.due_unix_seconds, None);
        assert_eq!(exhausted.admissible_at(0), Ok(()));
        assert_eq!(exhausted.first_admissible_second(), Ok(None));
        assert_eq!(exhausted.failure_selector, primary.failure_selector);

        // THE ENDS. A market that already reached one is not walked.
        for phase in [
            SourceResolutionPhaseV1::Resolved,
            SourceResolutionPhaseV1::FailureCommitted,
            SourceResolutionPhaseV1::Retired,
        ] {
            assert_eq!(
                deadline_failure_decision_v1(phase, 0, false, window, &domain),
                Err(DeadlineFailureErrorV1::AlreadyTerminal(phase))
            );
        }
        assert_eq!(
            DeadlineFailureArmV1::PrimaryDeadline.label(),
            "primary-deadline"
        );
        assert_eq!(
            DeadlineFailureArmV1::LadderExhausted.label(),
            "ladder-exhausted"
        );
    }
}
