//! Runtime-width mutable Source resolution state.

use dclutch_product_runtime_v2::{Error as ProductRuntimeError, ResultDomainV2};

use super::{
    ContentId, Error, MarketChildDeltaV1, RecoveryAttemptV2, RecoveryPolicyV2, Result,
    SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionRouteV1, WindowSpecV1,
    generated_source_resolution_state_v2 as generated,
};

/// The exclusive bound on the persisted `active_attempt` byte.
///
/// It used to be a bare `4` written here, a second bare `4` inside the Lean
/// record's own validity rule, and `RECOVERY_POLICY_MAX_ATTEMPTS_V2` emitted
/// from the policy's schema -- three authors of one number. It is now the
/// emitted one, and the Lean rule is defined as the policy's capacity rather
/// than as a numeral beside it, because an `active_attempt` the policy cannot
/// fund is an attempt nothing paid for.
const MAX_RECOVERY_ATTEMPTS_V2: u8 = generated::SOURCE_RESOLUTION_MAX_RECOVERY_ATTEMPTS_V2;

const _: () = assert!(
    MAX_RECOVERY_ATTEMPTS_V2 as usize == super::RECOVERY_POLICY_MAX_ATTEMPTS_V2,
    "the state's attempt bound and the policy's capacity are one number"
);

/// PDA seed material for one runtime-width Source resolution state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionPdaSeedsV2 {
    market: [u8; 32],
    generation_le: [u8; 8],
    bump: u8,
}

impl SourceResolutionPdaSeedsV2 {
    /// Return the exact, unhashed V2 PDA domain seed.
    pub const fn domain(self) -> &'static [u8] {
        super::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2
    }

    /// Return the exact Market-key seed.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return little-endian generation seed bytes.
    pub const fn generation_le(self) -> [u8; 8] {
        self.generation_le
    }

    /// Return the canonical PDA bump byte.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// Provider-neutral runtime-width terminal decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionDecisionV2 {
    route: SourceResolutionRouteV1,
    selector: u32,
    outcome_count: u32,
    resolution_evidence_id: ContentId,
    terminal_sequence: u64,
}

/// Structurally authenticated terminal facts used by retirement after Core
/// has already admitted the Product-bound terminal certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionTerminalProjectionV2 {
    route: SourceResolutionRouteV1,
    selector: u32,
    resolution_evidence_id: ContentId,
    terminal_sequence: u64,
}

impl SourceResolutionTerminalProjectionV2 {
    /// Provider-neutral primary/recovery/failure terminal route.
    pub const fn route(self) -> SourceResolutionRouteV1 {
        self.route
    }

    /// Native runtime-width terminal selector.
    pub const fn selector(self) -> u32 {
        self.selector
    }

    /// Exact accepted evidence content identity.
    pub const fn resolution_evidence_id(self) -> ContentId {
        self.resolution_evidence_id
    }

    /// Positive terminal replay sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }
}

impl SourceResolutionDecisionV2 {
    fn new(
        route: SourceResolutionRouteV1,
        selector: u32,
        outcome_count: u32,
        resolution_evidence_id: ContentId,
        terminal_sequence: u64,
    ) -> Result<Self> {
        if terminal_sequence == 0 {
            return Err(Error::ZeroSequence);
        }
        if outcome_count < 2 || selector >= outcome_count {
            return Err(Error::InvalidResultSelector);
        }
        Ok(Self {
            route,
            selector,
            outcome_count,
            resolution_evidence_id,
            terminal_sequence,
        })
    }

    /// Return the provider-neutral primary/recovery/failure route.
    pub const fn route(self) -> SourceResolutionRouteV1 {
        self.route
    }

    /// Return the native runtime-width Product selector without truncation.
    pub const fn selector(self) -> u32 {
        self.selector
    }

    /// Return the independently authenticated Product outcome count.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Return the exact accepted evidence content identity.
    pub const fn resolution_evidence_id(self) -> ContentId {
        self.resolution_evidence_id
    }

    /// Return the positive terminal replay sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }
}

/// The one thing a permissionless crank of the funded ladder can do.
///
/// The two variants are not two transitions. They are the two arms of one
/// decision taken at one moment -- the current window has closed, and either
/// the policy funds another attempt or it does not -- so a caller never chooses
/// between them and no rung has both available or neither.
///
/// Each arm carries the attempt whose funding pays for it. Entering attempt `n`
/// is paid by attempt `n`'s own allocation, which is the binding founding
/// established when the compartments were created; the exhaustion is paid by
/// the compartment configured by the policy itself, because no single attempt
/// owns the end of the ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryCrankV2 {
    /// The ladder entered a funded attempt.
    Advanced {
        /// Zero-based index of the attempt now active.
        attempt_index: u8,
        /// The attempt the ladder entered, whose allocation pays this crank.
        attempt: RecoveryAttemptV2,
    },
    /// The last funded window closed with nothing observed.
    Exhausted {
        /// Zero-based index of the attempt the ladder just spent.
        final_attempt_index: u8,
        /// The spent attempt, whose provider release names the dead route.
        final_attempt: RecoveryAttemptV2,
    },
}

/// Persisted Source state whose selector covers the full Product Runtime V2 domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionStateV2 {
    phase: SourceResolutionPhaseV1,
    active_attempt: u8,
    terminal_route: Option<SourceResolutionRouteV1>,
    pda_bump: u8,
    result_selector: u32,
    market: [u8; 32],
    generation: u64,
    material_id: ContentId,
    rent_beneficiary: [u8; 32],
    reopen_link_id: Option<ContentId>,
    resolution_evidence_id: Option<ContentId>,
    terminal_sequence: u64,
    resolved_at_unix_seconds: i64,
    retired_at_unix_seconds: i64,
}

/// Joined result of creating one V2 state and registering one Market child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionCreationPlanV2 {
    state: SourceResolutionStateV2,
    market_delta: MarketChildDeltaV1,
}

impl SourceResolutionCreationPlanV2 {
    /// Return the exact state to persist.
    pub const fn state(self) -> SourceResolutionStateV2 {
        self.state
    }

    /// Return the exactly-one Market registration to apply atomically.
    pub const fn market_delta(self) -> MarketChildDeltaV1 {
        self.market_delta
    }
}

impl SourceResolutionStateV2 {
    /// Begin a fresh primary state bound to one authenticated Market generation
    /// and exact `SourceMaterialV3` content digest.
    #[allow(clippy::too_many_arguments)]
    pub fn fresh(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        rent_beneficiary: [u8; 32],
        pda_bump: u8,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<SourceResolutionCreationPlanV2> {
        if is_zero(&market) || is_zero(&rent_beneficiary) || generation == 0 {
            return Err(Error::ZeroIdentifier);
        }
        let state = Self {
            phase: SourceResolutionPhaseV1::Primary,
            active_attempt: 0,
            terminal_route: None,
            pda_bump,
            result_selector: 0,
            market,
            generation,
            material_id,
            rent_beneficiary,
            reopen_link_id: None,
            resolution_evidence_id: None,
            terminal_sequence: 0,
            resolved_at_unix_seconds: 0,
            retired_at_unix_seconds: 0,
        };
        state.validate_shape()?;
        Ok(SourceResolutionCreationPlanV2 {
            state,
            market_delta: MarketChildDeltaV1::register(
                expected_market_child_count,
                authenticated_market_child_count,
            )?,
        })
    }

    /// Hostile-decode one exact Lean-owned 224-byte state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != generated::SOURCE_RESOLUTION_STATE_V2_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, generated::SOURCE_RESOLUTION_STATE_V2_MAGIC_OFFSET)?
            != generated::SOURCE_RESOLUTION_STATE_V2_MAGIC
        {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(
            bytes,
            generated::SOURCE_RESOLUTION_STATE_V2_VERSION_OFFSET,
        )?) != generated::SOURCE_RESOLUTION_STATE_V2_SCHEMA_VERSION
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            generated::SOURCE_RESOLUTION_STATE_V2_RESERVED_HEADER_OFFSET,
            2,
        )?;
        require_zero(
            bytes,
            generated::SOURCE_RESOLUTION_STATE_V2_RESERVED_SELECTOR_OFFSET,
            4,
        )?;
        require_zero(
            bytes,
            generated::SOURCE_RESOLUTION_STATE_V2_RESERVED_TAIL_OFFSET,
            8,
        )?;
        let route = byte(
            bytes,
            generated::SOURCE_RESOLUTION_STATE_V2_TERMINAL_ROUTE_OFFSET,
        )?;
        let value = Self {
            phase: SourceResolutionPhaseV1::decode(byte(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_PHASE_OFFSET,
            )?)?,
            active_attempt: byte(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_ACTIVE_ATTEMPT_OFFSET,
            )?,
            terminal_route: if route == 0 {
                None
            } else {
                Some(SourceResolutionRouteV1::decode(route)?)
            },
            pda_bump: byte(bytes, generated::SOURCE_RESOLUTION_STATE_V2_PDA_BUMP_OFFSET)?,
            result_selector: u32::from_le_bytes(array(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_SELECTOR_OFFSET,
            )?),
            market: array(bytes, generated::SOURCE_RESOLUTION_STATE_V2_MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_GENERATION_OFFSET,
            )?),
            material_id: content(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_MATERIAL_DIGEST_OFFSET,
            )?,
            rent_beneficiary: array(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_RENT_BENEFICIARY_OFFSET,
            )?,
            reopen_link_id: optional_content(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_REOPEN_LINK_OFFSET,
            )?,
            resolution_evidence_id: optional_content(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_RESOLUTION_EVIDENCE_OFFSET,
            )?,
            terminal_sequence: u64::from_le_bytes(array(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_TERMINAL_SEQUENCE_OFFSET,
            )?),
            resolved_at_unix_seconds: i64::from_le_bytes(array(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_RESOLVED_AT_OFFSET,
            )?),
            retired_at_unix_seconds: i64::from_le_bytes(array(
                bytes,
                generated::SOURCE_RESOLUTION_STATE_V2_RETIRED_AT_OFFSET,
            )?),
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode the one exact canonical state representation.
    #[must_use]
    pub fn to_bytes(self) -> [u8; generated::SOURCE_RESOLUTION_STATE_V2_BYTES] {
        let mut output = [0_u8; generated::SOURCE_RESOLUTION_STATE_V2_BYTES];
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_MAGIC_OFFSET,
            &generated::SOURCE_RESOLUTION_STATE_V2_MAGIC,
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_VERSION_OFFSET,
            &generated::SOURCE_RESOLUTION_STATE_V2_SCHEMA_VERSION.to_le_bytes(),
        );
        output[generated::SOURCE_RESOLUTION_STATE_V2_PHASE_OFFSET] = self.phase.byte();
        output[generated::SOURCE_RESOLUTION_STATE_V2_ACTIVE_ATTEMPT_OFFSET] = self.active_attempt;
        output[generated::SOURCE_RESOLUTION_STATE_V2_TERMINAL_ROUTE_OFFSET] =
            self.terminal_route.map_or(0, SourceResolutionRouteV1::byte);
        output[generated::SOURCE_RESOLUTION_STATE_V2_PDA_BUMP_OFFSET] = self.pda_bump;
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_SELECTOR_OFFSET,
            &self.result_selector.to_le_bytes(),
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_MARKET_OFFSET,
            &self.market,
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_MATERIAL_DIGEST_OFFSET,
            self.material_id.as_bytes(),
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_RENT_BENEFICIARY_OFFSET,
            &self.rent_beneficiary,
        );
        if let Some(value) = self.reopen_link_id {
            put(
                &mut output,
                generated::SOURCE_RESOLUTION_STATE_V2_REOPEN_LINK_OFFSET,
                value.as_bytes(),
            );
        }
        if let Some(value) = self.resolution_evidence_id {
            put(
                &mut output,
                generated::SOURCE_RESOLUTION_STATE_V2_RESOLUTION_EVIDENCE_OFFSET,
                value.as_bytes(),
            );
        }
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_TERMINAL_SEQUENCE_OFFSET,
            &self.terminal_sequence.to_le_bytes(),
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_RESOLVED_AT_OFFSET,
            &self.resolved_at_unix_seconds.to_le_bytes(),
        );
        put(
            &mut output,
            generated::SOURCE_RESOLUTION_STATE_V2_RETIRED_AT_OFFSET,
            &self.retired_at_unix_seconds.to_le_bytes(),
        );
        output
    }

    /// Map one exact normalized primary result through an independently
    /// authenticated Product Runtime V2 domain and commit the terminal state
    /// only after every check succeeds.
    ///
    /// `source_scale_exponent` is the source-to-result decimal shift the
    /// market's `StatisticSpecV1` declares. Every caller reads it from that
    /// record -- through the provider obligation where there is one -- and
    /// never from an adapter account, so the observation reaches the selector
    /// on the scale the founding declared rather than on whichever scale the
    /// feed happened to publish.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_primary_from_authenticated_domain(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV3,
        authenticated_product_record_digest: ContentId,
        domain: ResultDomainV2<'_>,
        resolution_evidence_id: ContentId,
        numerator: i128,
        denominator: u64,
        source_scale_exponent: i32,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV2> {
        self.validate_material_and_generation(material_id, expected_generation)?;
        material.authenticate_product_record(authenticated_product_record_digest)?;
        if self.phase != SourceResolutionPhaseV1::Primary || current_unix_seconds <= 0 {
            return Err(Error::InvalidRecoveryTransition);
        }
        let outcome_count = domain
            .outcome_count()
            .map_err(|_| Error::InvalidResultMap)?;
        let selector = domain
            .select_ordinary(numerator, denominator, source_scale_exponent)
            .map_err(|error| match error {
                // The two scale refusals are the market's own records
                // disagreeing about units, which is a different accusation
                // from a malformed or unordered result map and must not be
                // flattened into one: a reader who sees `InvalidResultMap`
                // goes looking at the cuts, and the cuts are fine.
                ProductRuntimeError::UnsupportedScale => Error::NonCanonicalSourceScale,
                ProductRuntimeError::ArithmeticOverflow => Error::ArithmeticOverflow,
                _ => Error::InvalidResultMap,
            })?;
        let decision = SourceResolutionDecisionV2::new(
            SourceResolutionRouteV1::Primary,
            selector,
            outcome_count,
            resolution_evidence_id,
            terminal_sequence,
        )?;
        let mut candidate = *self;
        candidate.phase = SourceResolutionPhaseV1::Resolved;
        candidate.active_attempt = 0;
        candidate.terminal_route = Some(SourceResolutionRouteV1::Primary);
        candidate.result_selector = selector;
        candidate.resolution_evidence_id = Some(resolution_evidence_id);
        candidate.terminal_sequence = terminal_sequence;
        candidate.resolved_at_unix_seconds = current_unix_seconds;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(decision)
    }

    /// Enter `Exhausted` because the primary window's own deadline passed.
    ///
    /// This is the V2 half of the liveness walk that `MAINNET_STATE_RELAY.md`
    /// §4.8 describes: the property it buys is that **a silent provider cannot
    /// make a market unresolvable**, only drive it to a pre-disclosed outcome.
    /// Without a transition out of `Primary` there is no such property, because
    /// `commit_failure_from_authenticated_domain` refuses anywhere but
    /// `Exhausted` and nothing could ever get there.
    ///
    /// The deadline is the window's own closed upper bound plus its liveness
    /// grace — the same `primary_deadline` the V1 view uses — and the comparison
    /// is strict, so the last admissible second for an honest resolution and the
    /// first admissible second for a failure are different seconds.
    ///
    /// A material carrying a recovery policy is refused here on purpose. A
    /// policy means the market bought named alternative sources, and skipping
    /// them would take an outcome away from the holders who paid for them; that
    /// walk is `FailNext` per leg, it must debit the leg's own funding
    /// allocation, and it belongs to the funded controller rather than to this
    /// transition.
    pub fn exhaust_after_primary_deadline(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV3,
        authenticated_window_spec_id: ContentId,
        window: WindowSpecV1,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<()> {
        self.validate_material_and_generation(material_id, expected_generation)?;
        // `window` is a value the caller authenticated against this identity by
        // digest, exactly as the terminal transition takes an authenticated
        // result domain. This crate hashes nothing and cannot re-derive it.
        if material.window_spec() != authenticated_window_spec_id {
            return Err(Error::LinkageMismatch);
        }
        if material.recovery_policy().is_some() {
            return Err(Error::RecoveryNotExhausted);
        }
        if self.phase != SourceResolutionPhaseV1::Primary || current_unix_seconds <= 0 {
            return Err(Error::InvalidRecoveryTransition);
        }
        let deadline = window
            .end_unix_seconds()
            .checked_add(i64::from(window.max_age_seconds()))
            .ok_or(Error::ArithmeticOverflow)?;
        if current_unix_seconds <= deadline {
            return Err(Error::DeadlineNotReached);
        }
        let mut candidate = *self;
        candidate.phase = SourceResolutionPhaseV1::Exhausted;
        candidate.active_attempt = 0;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(())
    }

    /// Commit the Product-owned explicit-failure selector from an independently
    /// authenticated Runtime V2 domain after Source has entered Exhausted.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_failure_from_authenticated_domain(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV3,
        authenticated_product_record_digest: ContentId,
        domain: ResultDomainV2<'_>,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV2> {
        self.validate_material_and_generation(material_id, expected_generation)?;
        material.authenticate_product_record(authenticated_product_record_digest)?;
        if self.phase != SourceResolutionPhaseV1::Exhausted || current_unix_seconds <= 0 {
            return Err(Error::RecoveryNotExhausted);
        }
        let outcome_count = domain
            .outcome_count()
            .map_err(|_| Error::InvalidResultMap)?;
        let selector = domain.failure_selector();
        let decision = SourceResolutionDecisionV2::new(
            SourceResolutionRouteV1::Failure,
            selector,
            outcome_count,
            material_id,
            terminal_sequence,
        )?;
        let mut candidate = *self;
        candidate.phase = SourceResolutionPhaseV1::FailureCommitted;
        candidate.active_attempt = 0;
        candidate.terminal_route = Some(SourceResolutionRouteV1::Failure);
        candidate.result_selector = selector;
        candidate.resolution_evidence_id = Some(material_id);
        candidate.terminal_sequence = terminal_sequence;
        candidate.resolved_at_unix_seconds = current_unix_seconds;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(decision)
    }

    /// One permissionless crank of the funded ordered-recovery ladder.
    ///
    /// This is the transition the machine did not have. `Phase::Recovery` was a
    /// phase the record could describe and no route could reach: the primary
    /// exhaustion above refuses a recovery-bearing material on purpose, and the
    /// failure commit only fires from `Exhausted`, so a market founded with a
    /// recovery policy had no terminal at all and every holder's principal sat
    /// in it. This is the way in.
    ///
    /// It is ONE transition rather than a family, and the two outcomes are the
    /// two arms of a single decision made at a single moment: the current
    /// window has closed, and either the policy funds another attempt (the
    /// ladder advances) or it does not (the ladder is exhausted). Nothing else
    /// can happen at that moment, which is what makes the walk a walk instead
    /// of a choice -- `Ladder.a_closed_recovery_window_has_exactly_one_move` in
    /// `SourceResolutionStateV2Abi.lean` is exactly that claim.
    ///
    /// What it does not do: move a lamport. The caller debits the compartment
    /// the returned attempt names, exactly as the deadline-failure walk debits
    /// the compartment the material names. An advance that cannot be paid for
    /// must not move the market either, so the outer plans the debit first.
    ///
    /// Timing is strict on both legs. On `Primary` the window is the material's
    /// own `WindowSpecV1` closed end plus its liveness grace -- the same
    /// `primary_deadline` the failure walk uses. On `Recovery` it is the active
    /// attempt's own committed absolute deadline. In both cases the last
    /// admissible second for an honest observation and the first admissible
    /// second for a crank are different seconds.
    #[allow(clippy::too_many_arguments)]
    pub fn crank_recovery_ladder(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV3,
        authenticated_window_spec_id: ContentId,
        window: WindowSpecV1,
        authenticated_recovery_policy_id: ContentId,
        policy: RecoveryPolicyV2,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<RecoveryCrankV2> {
        self.validate_material_and_generation(material_id, expected_generation)?;
        let active = self.authenticate_ladder(
            material,
            authenticated_window_spec_id,
            authenticated_recovery_policy_id,
            policy,
        )?;
        if current_unix_seconds <= 0 {
            return Err(Error::InvalidRecoveryTransition);
        }

        // The window whose closing this crank claims. On `Primary` it is the
        // market's own; on `Recovery` it is the active attempt's, which is why
        // an attempt index nothing funds has no window and can never close.
        let due = match self.phase {
            SourceResolutionPhaseV1::Primary => window
                .end_unix_seconds()
                .checked_add(i64::from(window.max_age_seconds()))
                .ok_or(Error::ArithmeticOverflow)?,
            SourceResolutionPhaseV1::Recovery => active
                .ok_or(Error::InvalidRecoveryTransition)?
                .1
                .deadline_unix_seconds(),
            _ => return Err(Error::InvalidRecoveryTransition),
        };
        if current_unix_seconds <= due {
            return Err(Error::DeadlineNotReached);
        }

        let entering = match self.phase {
            SourceResolutionPhaseV1::Primary => 0_u8,
            _ => active
                .ok_or(Error::InvalidRecoveryTransition)?
                .0
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
        };

        // Funded and enterable are one word. `attempt` refuses any index the
        // policy does not fund, so the advance arm cannot be reached for a leg
        // nobody paid for and the exhaustion arm is its exact complement.
        match policy.attempt(entering) {
            Ok(attempt) => {
                let mut candidate = *self;
                candidate.phase = SourceResolutionPhaseV1::Recovery;
                candidate.active_attempt = entering;
                candidate.validate_shape()?;
                *self = candidate;
                Ok(RecoveryCrankV2::Advanced {
                    attempt_index: entering,
                    attempt,
                })
            }
            Err(_) => {
                let (final_index, final_attempt) = active.ok_or(Error::RecoveryNotExhausted)?;
                let mut candidate = *self;
                candidate.phase = SourceResolutionPhaseV1::Exhausted;
                candidate.active_attempt = 0;
                candidate.validate_shape()?;
                *self = candidate;
                Ok(RecoveryCrankV2::Exhausted {
                    final_attempt_index: final_index,
                    final_attempt,
                })
            }
        }
    }

    /// Map one exact normalized recovery result through an independently
    /// authenticated Product Runtime V2 domain.
    ///
    /// The recovery leg is stricter than the primary one about time, and
    /// deliberately so. The primary window lives in a separate authenticated
    /// `WindowSpecV1` record and its freshness is the provider outer's to
    /// enforce; a recovery attempt's deadline is a field of the policy this
    /// transition already holds, so refusing a late capture here costs nothing
    /// and closes the gap by which a crank and a capture could both claim the
    /// same second.
    ///
    /// The capture must also name the active attempt's OWN source spec and
    /// provider release. A market that bought a named alternative feed is not a
    /// market where any feed may answer at any time: attempt `n` is the only
    /// source admissible while the ladder stands on `n`.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_recovery_from_authenticated_domain(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV3,
        authenticated_window_spec_id: ContentId,
        authenticated_product_record_digest: ContentId,
        authenticated_recovery_policy_id: ContentId,
        policy: RecoveryPolicyV2,
        authenticated_attempt_source_spec_id: ContentId,
        authenticated_provider_release_id: ContentId,
        domain: ResultDomainV2<'_>,
        resolution_evidence_id: ContentId,
        numerator: i128,
        denominator: u64,
        source_scale_exponent: i32,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV2> {
        self.validate_material_and_generation(material_id, expected_generation)?;
        material.authenticate_product_record(authenticated_product_record_digest)?;
        let active = self.authenticate_ladder(
            material,
            authenticated_window_spec_id,
            authenticated_recovery_policy_id,
            policy,
        )?;
        if self.phase != SourceResolutionPhaseV1::Recovery || current_unix_seconds <= 0 {
            return Err(Error::InvalidRecoveryTransition);
        }
        let (_, attempt) = active.ok_or(Error::InvalidRecoveryTransition)?;
        if attempt.source_spec_id() != authenticated_attempt_source_spec_id
            || attempt.provider_release_id() != authenticated_provider_release_id
        {
            return Err(Error::LinkageMismatch);
        }
        if current_unix_seconds > attempt.deadline_unix_seconds() {
            return Err(Error::DeadlineElapsed);
        }
        let outcome_count = domain
            .outcome_count()
            .map_err(|_| Error::InvalidResultMap)?;
        let selector = domain
            .select_ordinary(numerator, denominator, source_scale_exponent)
            .map_err(|error| match error {
                ProductRuntimeError::UnsupportedScale => Error::NonCanonicalSourceScale,
                ProductRuntimeError::ArithmeticOverflow => Error::ArithmeticOverflow,
                _ => Error::InvalidResultMap,
            })?;
        let decision = SourceResolutionDecisionV2::new(
            SourceResolutionRouteV1::Recovery,
            selector,
            outcome_count,
            resolution_evidence_id,
            terminal_sequence,
        )?;
        let mut candidate = *self;
        candidate.phase = SourceResolutionPhaseV1::Resolved;
        candidate.active_attempt = 0;
        candidate.terminal_route = Some(SourceResolutionRouteV1::Recovery);
        candidate.result_selector = selector;
        candidate.resolution_evidence_id = Some(resolution_evidence_id);
        candidate.terminal_sequence = terminal_sequence;
        candidate.resolved_at_unix_seconds = current_unix_seconds;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(decision)
    }

    /// The compartment configuration the next crank of the ladder will spend.
    ///
    /// A caller has to select the ledger row BEFORE the crank runs -- one
    /// account holds all three of a market's Resolution compartments and each
    /// is found by its own pinned configuration, not by a position -- so the
    /// answer would otherwise have two authors: the outer that selects the row
    /// and the crank that returns the attempt. It has one, and the two are
    /// welded by the planner comparing the selected entry's configuration
    /// against the identity the crank hands back.
    ///
    /// Entering attempt `n` is paid by attempt `n`'s own allocation. The end of
    /// the ladder belongs to no single attempt, so the policy's own digest
    /// configures it -- exactly the binding founding established when the three
    /// compartments were created.
    pub fn next_crank_funding_config(
        self,
        authenticated_recovery_policy_id: ContentId,
        policy: RecoveryPolicyV2,
    ) -> Result<ContentId> {
        let entering = match self.phase {
            SourceResolutionPhaseV1::Primary => 0_u8,
            SourceResolutionPhaseV1::Recovery => self
                .active_attempt
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            _ => return Err(Error::InvalidRecoveryTransition),
        };
        match policy.attempt(entering) {
            Ok(attempt) => Ok(attempt.funding_allocation_id()),
            Err(_) => Ok(authenticated_recovery_policy_id),
        }
    }

    /// Join the material, its window and its recovery policy, and project the
    /// attempt the state currently stands on.
    ///
    /// `None` means the state is not on `Recovery`, which is a different fact
    /// from "the policy does not fund this index" -- the second cannot happen,
    /// because the record's own canonicity rule bounds `active_attempt` and the
    /// join below refuses any index the policy will not hand back.
    fn authenticate_ladder(
        self,
        material: SourceMaterialV3,
        authenticated_window_spec_id: ContentId,
        authenticated_recovery_policy_id: ContentId,
        policy: RecoveryPolicyV2,
    ) -> Result<Option<(u8, RecoveryAttemptV2)>> {
        // Both records reached this crate as values a caller authenticated by
        // digest, exactly as the terminal transitions take an authenticated
        // result domain. This crate hashes nothing and cannot re-derive them.
        if material.window_spec() != authenticated_window_spec_id {
            return Err(Error::LinkageMismatch);
        }
        if material.recovery_policy() != Some(authenticated_recovery_policy_id) {
            return Err(Error::LinkageMismatch);
        }
        match self.phase {
            SourceResolutionPhaseV1::Recovery => {
                let attempt = policy.attempt(self.active_attempt)?;
                Ok(Some((self.active_attempt, attempt)))
            }
            _ => Ok(None),
        }
    }

    /// Reconstruct the terminal decision against the independently
    /// authenticated Product Runtime V2 outcome count.
    pub fn decision(self, authenticated_outcome_count: u32) -> Result<SourceResolutionDecisionV2> {
        if !matches!(
            self.phase,
            SourceResolutionPhaseV1::Resolved
                | SourceResolutionPhaseV1::FailureCommitted
                | SourceResolutionPhaseV1::Retired
        ) {
            return Err(Error::InvalidRecoveryTransition);
        }
        SourceResolutionDecisionV2::new(
            self.terminal_route.ok_or(Error::NonCanonicalState)?,
            self.result_selector,
            authenticated_outcome_count,
            self.resolution_evidence_id
                .ok_or(Error::NonCanonicalState)?,
            self.terminal_sequence,
        )
    }

    /// Project already-admitted terminal facts for retirement without
    /// accepting a caller-authored Product outcome count. Close must rejoin
    /// these facts to Core's admitted Product root, selector, and receipt.
    pub fn terminal_projection(self) -> Result<SourceResolutionTerminalProjectionV2> {
        if !matches!(
            self.phase,
            SourceResolutionPhaseV1::Resolved
                | SourceResolutionPhaseV1::FailureCommitted
                | SourceResolutionPhaseV1::Retired
        ) {
            return Err(Error::InvalidRecoveryTransition);
        }
        Ok(SourceResolutionTerminalProjectionV2 {
            route: self.terminal_route.ok_or(Error::NonCanonicalState)?,
            selector: self.result_selector,
            resolution_evidence_id: self
                .resolution_evidence_id
                .ok_or(Error::NonCanonicalState)?,
            terminal_sequence: self.terminal_sequence,
        })
    }

    /// Retire a terminal state and return the exactly-one Market child delta.
    pub fn retire(
        &mut self,
        generation: u64,
        current_unix_seconds: i64,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<MarketChildDeltaV1> {
        if generation != self.generation
            || !matches!(
                self.phase,
                SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
            )
            || current_unix_seconds < self.resolved_at_unix_seconds
        {
            return Err(Error::InvalidRecoveryTransition);
        }
        let delta = MarketChildDeltaV1::retire(
            expected_market_child_count,
            authenticated_market_child_count,
        )?;
        let mut candidate = *self;
        candidate.phase = SourceResolutionPhaseV1::Retired;
        candidate.retired_at_unix_seconds = current_unix_seconds;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(delta)
    }

    /// Return exact V2 PDA derivation seeds.
    pub const fn pda_seeds(self) -> SourceResolutionPdaSeedsV2 {
        SourceResolutionPdaSeedsV2 {
            market: self.market,
            generation_le: self.generation.to_le_bytes(),
            bump: self.pda_bump,
        }
    }

    /// Return the current persisted lifecycle phase.
    pub const fn phase(self) -> SourceResolutionPhaseV1 {
        self.phase
    }

    /// Return the bound Market account key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact `SourceMaterialV3` content digest.
    pub const fn material_id(self) -> ContentId {
        self.material_id
    }

    /// Return the pre-existing RentCredit beneficiary.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the optional authenticated reopen-link identity.
    pub const fn reopen_link_id(self) -> Option<ContentId> {
        self.reopen_link_id
    }

    fn validate_material_and_generation(
        self,
        material_id: ContentId,
        expected_generation: u64,
    ) -> Result<()> {
        if self.material_id != material_id || self.generation != expected_generation {
            return Err(Error::StateBindingMismatch);
        }
        Ok(())
    }

    fn validate_shape(self) -> Result<()> {
        if is_zero(&self.market) || is_zero(&self.rent_beneficiary) || self.generation == 0 {
            return Err(Error::ZeroIdentifier);
        }
        match self.phase {
            SourceResolutionPhaseV1::Primary | SourceResolutionPhaseV1::Exhausted => {
                if self.active_attempt != 0
                    || self.terminal_route.is_some()
                    || self.result_selector != 0
                    || self.resolution_evidence_id.is_some()
                    || self.terminal_sequence != 0
                    || self.resolved_at_unix_seconds != 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::Recovery => {
                if self.active_attempt >= MAX_RECOVERY_ATTEMPTS_V2
                    || self.terminal_route.is_some()
                    || self.result_selector != 0
                    || self.resolution_evidence_id.is_some()
                    || self.terminal_sequence != 0
                    || self.resolved_at_unix_seconds != 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::Resolved => {
                if !matches!(
                    self.terminal_route,
                    Some(SourceResolutionRouteV1::Primary | SourceResolutionRouteV1::Recovery)
                ) || self.active_attempt != 0
                    || self.resolution_evidence_id.is_none()
                    || self.terminal_sequence == 0
                    || self.resolved_at_unix_seconds <= 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::FailureCommitted => {
                if self.terminal_route != Some(SourceResolutionRouteV1::Failure)
                    || self.active_attempt != 0
                    || self.resolution_evidence_id.is_none()
                    || self.terminal_sequence == 0
                    || self.resolved_at_unix_seconds <= 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::Retired => {
                if self.terminal_route.is_none()
                    || self.active_attempt != 0
                    || self.resolution_evidence_id.is_none()
                    || self.terminal_sequence == 0
                    || self.resolved_at_unix_seconds <= 0
                    || self.retired_at_unix_seconds < self.resolved_at_unix_seconds
                {
                    return Err(Error::NonCanonicalState);
                }
            }
        }
        Ok(())
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

fn optional_content(bytes: &[u8], offset: usize) -> Result<Option<ContentId>> {
    let value = array(bytes, offset)?;
    if is_zero(&value) {
        Ok(None)
    } else {
        ContentId::new(value).map(Some)
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if bytes
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalReservedBytes)
    }
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(value.iter().copied()) {
        *destination = source;
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::generated_source_resolution_state_v2::{
        SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2_GENERATED, SOURCE_RESOLUTION_STATE_V2_FRESH_EXAMPLE,
        SOURCE_RESOLUTION_STATE_V2_REFUSAL_CORPUS, SOURCE_RESOLUTION_STATE_V2_REFUSAL_COUNT,
        SOURCE_RESOLUTION_STATE_V2_WIDE_TERMINAL_EXAMPLE,
    };
    use dclutch_product_runtime_v2::{ContentId as ProductContentId, ResultDomainInputV2};

    fn id(tag: u8) -> ContentId {
        ContentId::new(key(tag)).expect("nonzero")
    }

    fn key(tag: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = tag;
        value
    }

    fn product_id(tag: u8) -> ProductContentId {
        let mut value = [0_u8; 32];
        value[0] = tag;
        ProductContentId::new(value).expect("nonzero")
    }

    fn material(product: ContentId) -> SourceMaterialV3 {
        SourceMaterialV3::explicitly_unbounded(product, id(4), id(5), id(6), None, id(7))
    }

    fn runtime_domain_bytes(region_count: u32) -> alloc::vec::Vec<u8> {
        let cuts: alloc::vec::Vec<i128> = (1..region_count).map(i128::from).collect();
        let input = ResultDomainInputV2 {
            product_id: product_id(1),
            coordinate_domain_id: product_id(2),
            result_unit_id: product_id(3),
            liability_basis_id: product_id(4),
            representation_release_id: product_id(5),
            mapping_release_id: product_id(6),
            cut_denominator: 1,
            cuts: &cuts,
        };
        let cut_count = usize::try_from(region_count - 1).expect("cut count");
        let mut output = alloc::vec![
            0_u8;
            dclutch_product_runtime_v2::result_domain_record_bytes(cut_count)
                .expect("width")
        ];
        dclutch_product_runtime_v2::compile_result_domain_v2(input, &mut output).expect("domain");
        output
    }

    /// A terminal window sells the closed period `[end - width, end]`, and the
    /// deadline the walk waits for is its upper bound plus the window's own
    /// liveness grace. Only the upper bound is load-bearing here, but these
    /// windows are given real width on purpose: the walk must not start
    /// depending on a degeneracy that no market on a real provider cadence has.
    fn terminal_window(source_spec: ContentId, end: i64, grace: u32) -> WindowSpecV1 {
        WindowSpecV1::new(
            source_spec,
            crate::WindowKind::Terminal,
            end - 600,
            end,
            grace,
            1,
            id(9),
        )
        .expect("terminal window")
    }

    #[test]
    fn a_silent_provider_cannot_make_a_market_unresolvable() {
        // The property, executed. Before the deadline the market is still live
        // and the walk refuses; one second after it, anyone may drive the Source
        // to Exhausted, from where the Product's own failure selector is
        // reachable. Without this transition `commit_failure_from_authenticated_domain`
        // is unreachable and a silent provider bricks the market instead of
        // costing it a pre-disclosed outcome.
        let product = id(3);
        let material = material(product);
        let window = terminal_window(id(4), 1_000_000, 600);
        let deadline = 1_000_600;
        let mut state = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        assert_eq!(
            state.exhaust_after_primary_deadline(id(2), material, id(5), window, 9, deadline),
            Err(Error::DeadlineNotReached),
            "the last admissible second for an honest resolution is not a failure second"
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Primary);
        assert_eq!(
            state.exhaust_after_primary_deadline(id(2), material, id(5), window, 9, deadline + 1),
            Ok(())
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Exhausted);

        let domain_bytes = runtime_domain_bytes(2);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let decision = state
            .commit_failure_from_authenticated_domain(
                id(2),
                material,
                product,
                domain,
                9,
                deadline + 2,
                1,
            )
            .expect("the pre-disclosed outcome is reachable");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::FailureCommitted);
        assert_eq!(decision.selector(), domain.failure_selector());
        assert_eq!(decision.route(), SourceResolutionRouteV1::Failure);
    }

    #[test]
    fn the_deadline_walk_refuses_a_market_that_bought_alternative_sources() {
        // A recovery policy means the market paid for named alternative sources.
        // Skipping straight to failure would take an outcome away from the
        // holders who paid for it, so this transition refuses and the funded
        // per-leg walk owns that case.
        let with_recovery =
            SourceMaterialV3::explicitly_unbounded(id(3), id(4), id(5), id(6), Some(id(8)), id(7));
        let window = terminal_window(id(4), 1_000_000, 600);
        let mut state = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        assert_eq!(
            state.exhaust_after_primary_deadline(id(2), with_recovery, id(5), window, 9, 2_000_000),
            Err(Error::RecoveryNotExhausted)
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Primary);
    }

    /// The two-source market: a primary window and exactly one funded
    /// alternative, which is the shape founding admits and the shape the
    /// campaign walks.
    fn two_source_material() -> (SourceMaterialV3, RecoveryPolicyV2, ContentId) {
        let policy = RecoveryPolicyV2::new(
            id(0x60),
            [
                Some(
                    RecoveryAttemptV2::new(id(0x61), id(0x62), RECOVERY_DEADLINE, id(0x63))
                        .expect("attempt"),
                ),
                None,
                None,
                None,
            ],
            1,
        )
        .expect("policy");
        let policy_id = id(0x64);
        let material = SourceMaterialV3::explicitly_unbounded(
            id(3),
            id(4),
            id(5),
            id(6),
            Some(policy_id),
            id(7),
        );
        (material, policy, policy_id)
    }

    const PRIMARY_DEADLINE: i64 = 1_000_600;
    const RECOVERY_DEADLINE: i64 = 1_002_000;

    fn ladder_state() -> SourceResolutionStateV2 {
        SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state()
    }

    #[test]
    fn the_funded_ladder_walks_primary_recovery_exhausted_failure() {
        // The whole property, executed on the shape founding admits. A market
        // that bought one named alternative source cannot be terminalized at
        // all without this walk: `exhaust_after_primary_deadline` refuses it by
        // name, so before the crank existed every holder's principal sat in a
        // market with no exit.
        let (material, policy, policy_id) = two_source_material();
        let window = terminal_window(id(4), 1_000_000, 600);
        let mut state = ladder_state();

        // Primary window still open: the crank refuses on the second the honest
        // observation is still admissible.
        assert_eq!(
            state.crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                PRIMARY_DEADLINE
            ),
            Err(Error::DeadlineNotReached)
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Primary);

        // One second later the ladder advances onto the funded alternative.
        let crank = state
            .crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                PRIMARY_DEADLINE + 1,
            )
            .expect("the funded alternative is enterable");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Recovery);
        assert_eq!(
            crank,
            RecoveryCrankV2::Advanced {
                attempt_index: 0,
                attempt: policy.attempt(0).expect("attempt"),
            },
            "the entered attempt names the compartment that pays for entering it"
        );

        // The alternative's own window is open until its committed deadline.
        assert_eq!(
            state.crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                RECOVERY_DEADLINE
            ),
            Err(Error::DeadlineNotReached)
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Recovery);

        // Past it, the same one transition exhausts rather than advances --
        // there is no second attempt to enter.
        let crank = state
            .crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                RECOVERY_DEADLINE + 1,
            )
            .expect("the last funded window closed");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Exhausted);
        assert_eq!(
            crank,
            RecoveryCrankV2::Exhausted {
                final_attempt_index: 0,
                final_attempt: policy.attempt(0).expect("attempt"),
            }
        );

        // And `Exhausted` is where the existing failure commit already begins,
        // so the ladder added a way in and changed no way out.
        let domain_bytes = runtime_domain_bytes(2);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let decision = state
            .commit_failure_from_authenticated_domain(
                id(2),
                material,
                id(3),
                domain,
                9,
                RECOVERY_DEADLINE + 2,
                1,
            )
            .expect("the pre-disclosed outcome is reachable at the end of the ladder");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::FailureCommitted);
        assert_eq!(decision.route(), SourceResolutionRouteV1::Failure);
        assert_eq!(decision.selector(), domain.failure_selector());
    }

    #[test]
    fn the_alternative_source_observed_inside_its_window_resolves() {
        // The honest recovery branch. The market bought the alternative and the
        // alternative answered, so the holders get a real outcome rather than
        // the failure cell, and the terminal route records that it came from
        // recovery rather than from the primary.
        let (material, policy, policy_id) = two_source_material();
        let window = terminal_window(id(4), 1_000_000, 600);
        let domain_bytes = runtime_domain_bytes(3);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let mut state = ladder_state();
        state
            .crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                PRIMARY_DEADLINE + 1,
            )
            .expect("advance");

        let decision = state
            .resolve_recovery_from_authenticated_domain(
                id(2),
                material,
                id(5),
                id(3),
                policy_id,
                policy,
                id(0x61),
                id(0x62),
                domain,
                id(0x70),
                0,
                1,
                0,
                9,
                RECOVERY_DEADLINE,
                4,
            )
            .expect("the alternative answered inside its own window");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
        assert_eq!(decision.route(), SourceResolutionRouteV1::Recovery);
        assert!(decision.selector() < domain.failure_selector());
        assert_eq!(
            state.active_attempt, 0,
            "a terminal state carries no attempt"
        );
    }

    #[test]
    fn a_capture_against_the_wrong_attempts_source_refuses() {
        // A market that bought a named alternative is not a market where any
        // feed may answer at any time. The active attempt's own source spec and
        // provider release are the only pair admissible while the ladder stands
        // on it.
        let (material, policy, policy_id) = two_source_material();
        let window = terminal_window(id(4), 1_000_000, 600);
        let domain_bytes = runtime_domain_bytes(3);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let mut state = ladder_state();
        state
            .crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                PRIMARY_DEADLINE + 1,
            )
            .expect("advance");

        for (source_spec, release, why) in [
            (id(0x99), id(0x62), "a foreign source spec"),
            (id(0x61), id(0x99), "a foreign provider release"),
            (
                id(4),
                id(0x62),
                "the PRIMARY source spec, which is the near miss",
            ),
        ] {
            assert_eq!(
                state.resolve_recovery_from_authenticated_domain(
                    id(2),
                    material,
                    id(5),
                    id(3),
                    policy_id,
                    policy,
                    source_spec,
                    release,
                    domain,
                    id(0x70),
                    0,
                    1,
                    0,
                    9,
                    RECOVERY_DEADLINE,
                    4,
                ),
                Err(Error::LinkageMismatch),
                "{why} must not resolve a market standing on attempt 0"
            );
            assert_eq!(state.phase(), SourceResolutionPhaseV1::Recovery);
        }

        // And the alternative's own window closes: a capture one second past
        // the committed deadline is the crank's second, not the capture's.
        assert_eq!(
            state.resolve_recovery_from_authenticated_domain(
                id(2),
                material,
                id(5),
                id(3),
                policy_id,
                policy,
                id(0x61),
                id(0x62),
                domain,
                id(0x70),
                0,
                1,
                0,
                9,
                RECOVERY_DEADLINE + 1,
                4,
            ),
            Err(Error::DeadlineElapsed)
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Recovery);
    }

    #[test]
    fn the_ladder_refuses_a_policy_the_material_does_not_name() {
        // The policy reaches this crate as a value the caller authenticated by
        // digest. Presenting a well-formed policy the material never selected is
        // the whole substitution attack, and it is one comparison.
        let no_recovery = material(id(3));
        let (material, policy, policy_id) = two_source_material();
        let window = terminal_window(id(4), 1_000_000, 600);
        let mut state = ladder_state();
        assert_eq!(
            state.crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                id(0x9a),
                policy,
                9,
                PRIMARY_DEADLINE + 1
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            state.crank_recovery_ladder(
                id(2),
                material,
                id(0x9b),
                window,
                policy_id,
                policy,
                9,
                PRIMARY_DEADLINE + 1
            ),
            Err(Error::LinkageMismatch)
        );
        // A material that bought no alternatives has no ladder to crank at all:
        // it selects no policy, so no policy authenticates against it. The
        // no-recovery market keeps the primary exhaustion it already had.
        assert_eq!(
            state.crank_recovery_ladder(
                id(2),
                no_recovery,
                id(5),
                window,
                policy_id,
                policy,
                9,
                PRIMARY_DEADLINE + 1
            ),
            Err(Error::LinkageMismatch),
            "a material that selects no policy cannot be walked with one"
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Primary);
    }

    #[test]
    fn the_ladder_cannot_be_cranked_out_of_a_terminal_state() {
        // Once the market is decided the crank has nothing to move, whatever
        // deadline has passed. Without this the ladder would be a route by
        // which a late crank overwrites a real observation.
        let (material, policy, policy_id) = two_source_material();
        let window = terminal_window(id(4), 1_000_000, 600);
        let domain_bytes = runtime_domain_bytes(3);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let mut state = ladder_state();
        state
            .resolve_primary_from_authenticated_domain(
                id(2),
                material,
                id(3),
                domain,
                id(0x71),
                0,
                1,
                0,
                9,
                1_000_000,
                3,
            )
            .expect("the primary answered");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
        assert_eq!(
            state.crank_recovery_ladder(
                id(2),
                material,
                id(5),
                window,
                policy_id,
                policy,
                9,
                RECOVERY_DEADLINE + 1,
            ),
            Err(Error::InvalidRecoveryTransition)
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
    }

    #[test]
    fn the_state_attempt_bound_is_the_policys_capacity() {
        // Three files used to write the number 4. The state's bound is now the
        // emitted policy capacity, and `RecoveryPolicyV2` refuses to describe a
        // ladder wider than the record can carry -- so an `active_attempt` the
        // policy cannot fund is unrepresentable rather than merely unreachable.
        assert_eq!(
            usize::from(MAX_RECOVERY_ATTEMPTS_V2),
            crate::RECOVERY_POLICY_MAX_ATTEMPTS_V2
        );
        let attempt = RecoveryAttemptV2::new(id(0x61), id(0x62), RECOVERY_DEADLINE, id(0x63))
            .expect("attempt");
        assert_eq!(
            RecoveryPolicyV2::new(id(0x60), [Some(attempt), None, None, None], 0),
            Err(Error::RecoveryExceedsCapacity),
            "a ladder with no funded attempt is not a ladder"
        );
        assert_eq!(
            RecoveryPolicyV2::new(
                id(0x60),
                [Some(attempt), None, None, None],
                MAX_RECOVERY_ATTEMPTS_V2 + 1
            ),
            Err(Error::RecoveryExceedsCapacity)
        );
    }

    #[test]
    fn the_deadline_walk_refuses_a_window_the_material_does_not_name() {
        let material = material(id(3));
        let window = terminal_window(id(4), 1_000_000, 600);
        let mut state = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        assert_eq!(
            state.exhaust_after_primary_deadline(id(2), material, id(90), window, 9, 2_000_000),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(
            state.exhaust_after_primary_deadline(id(91), material, id(5), window, 9, 2_000_000),
            Err(Error::StateBindingMismatch)
        );
        assert_eq!(
            state.exhaust_after_primary_deadline(id(2), material, id(5), window, 8, 2_000_000),
            Err(Error::StateBindingMismatch)
        );
    }

    #[test]
    fn a_resolved_market_cannot_be_walked_to_failure_afterwards() {
        // The bound that matters most: once a real observation has resolved the
        // market, the deadline passing must not be able to overwrite it.
        let product = id(3);
        let material = material(product);
        let window = terminal_window(id(4), 1_000_000, 600);
        let domain_bytes = runtime_domain_bytes(2);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let mut state = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        state
            .resolve_primary_from_authenticated_domain(
                id(2),
                material,
                product,
                domain,
                id(20),
                0,
                1,
                0,
                9,
                999_999,
                1,
            )
            .expect("primary resolution");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
        assert_eq!(
            state.exhaust_after_primary_deadline(id(2), material, id(5), window, 9, 9_000_000),
            Err(Error::InvalidRecoveryTransition)
        );
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
    }

    #[test]
    fn generated_layout_examples_and_refusals_agree() {
        assert_eq!(
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2_GENERATED,
            super::super::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2
        );
        let fresh = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        assert_eq!(fresh.to_bytes(), SOURCE_RESOLUTION_STATE_V2_FRESH_EXAMPLE);
        assert_eq!(
            SourceResolutionStateV2::decode(&fresh.to_bytes()),
            Ok(fresh)
        );
        assert_eq!(
            SOURCE_RESOLUTION_STATE_V2_REFUSAL_CORPUS.len(),
            SOURCE_RESOLUTION_STATE_V2_REFUSAL_COUNT
        );
        for hostile in SOURCE_RESOLUTION_STATE_V2_REFUSAL_CORPUS {
            assert!(SourceResolutionStateV2::decode(&hostile).is_err());
        }
    }

    #[test]
    fn selector_above_u8_is_preserved_and_bounded_by_authenticated_count() {
        let product = id(1);
        let material = material(product);
        let domain_bytes = runtime_domain_bytes(257);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let mut state = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        let decision = state
            .resolve_primary_from_authenticated_domain(
                id(2),
                material,
                product,
                domain,
                id(8),
                257,
                1,
                0,
                9,
                100,
                1,
            )
            .expect("wide decision");
        assert_eq!(decision.selector(), 256);
        assert_eq!(decision.outcome_count(), 258);
        assert_eq!(state.decision(258), Ok(decision));
        assert_eq!(state.decision(256), Err(Error::InvalidResultSelector));
        assert_eq!(
            SourceResolutionStateV2::decode(&state.to_bytes()),
            Ok(state)
        );
    }

    #[test]
    fn product_substitution_and_failed_map_are_atomic() {
        let product = id(1);
        let material = material(product);
        let domain_bytes = runtime_domain_bytes(2);
        let domain = ResultDomainV2::decode(&domain_bytes).expect("domain");
        let initial = SourceResolutionStateV2::fresh(key(1), 9, id(2), key(3), 7, 0, 0)
            .expect("fresh")
            .state();
        let mut state = initial;
        assert_eq!(
            state.resolve_primary_from_authenticated_domain(
                id(2),
                material,
                id(9),
                domain,
                id(8),
                1,
                1,
                0,
                9,
                100,
                1,
            ),
            Err(Error::LinkageMismatch)
        );
        assert_eq!(state, initial);
        assert_eq!(
            state.resolve_primary_from_authenticated_domain(
                id(2),
                material,
                product,
                domain,
                id(8),
                1,
                0,
                0,
                9,
                100,
                1,
            ),
            Err(Error::InvalidResultMap)
        );
        assert_eq!(state, initial);
    }

    #[test]
    fn generated_wide_terminal_is_not_a_u8_projection() {
        let state =
            SourceResolutionStateV2::decode(&SOURCE_RESOLUTION_STATE_V2_WIDE_TERMINAL_EXAMPLE)
                .expect("wide state");
        assert_eq!(state.decision(258).expect("decision").selector(), 257);
        assert_eq!(state.decision(257), Err(Error::InvalidResultSelector));
    }
}
