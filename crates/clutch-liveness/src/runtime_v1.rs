// SPDX-License-Identifier: AGPL-3.0-or-later
//! Complete present-funding contract for one finite protocol lifecycle.
//!
//! This module has no fee, volume, collateral, Hoard, token-price, clock,
//! account-memory, or CPI input. An adapter can admit the lifecycle only after
//! seven distinct accounts contain the exact native lamports needed for their
//! immutable work and rent compartments. Initial prefunds and later surplus
//! are donations owned only by the immutable neutral sink.

use super::Id;

/// Local semantic-body magic. This is not a global account discriminator.
pub const RUNTIME_LIVENESS_POLICY_MAGIC_V1: [u8; 8] = *b"DCLPOL01";
/// Local semantic-body magic. This is not a global account discriminator.
pub const RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1: [u8; 8] = *b"DCLACC01";
/// Exact local policy/account semantic version.
pub const RUNTIME_LIVENESS_VERSION_V1: u16 = 1;
/// Number of independently owned mandatory work compartments.
pub const RUNTIME_COMPARTMENT_COUNT_V1: usize = 7;
/// Number of frozen terminal-path bounds.
pub const RUNTIME_TERMINAL_PATH_COUNT_V1: usize = 4;
/// Exact encoded bytes in [`RuntimeLivenessPolicyV1`].
pub const RUNTIME_LIVENESS_POLICY_BYTES_V1: usize = 404;
/// Exact encoded bytes in [`RuntimeCompartmentV1`].
pub const RUNTIME_LIVENESS_ACCOUNT_BYTES_V1: usize = 320;

/// Canonical compartment order used in policies, bundles, and codecs.
pub const RUNTIME_COMPARTMENT_ORDER_V1: [RuntimeCompartmentKindV1;
    RUNTIME_COMPARTMENT_COUNT_V1] = [
    RuntimeCompartmentKindV1::Source,
    RuntimeCompartmentKindV1::Candidate,
    RuntimeCompartmentKindV1::Clearing,
    RuntimeCompartmentKindV1::Settlement,
    RuntimeCompartmentKindV1::Resolution,
    RuntimeCompartmentKindV1::Retirement,
    RuntimeCompartmentKindV1::Recovery,
];

/// Canonical terminal-path order used in the policy codec.
pub const RUNTIME_TERMINAL_PATH_ORDER_V1: [RuntimeTerminalPathKindV1;
    RUNTIME_TERMINAL_PATH_COUNT_V1] = [
    RuntimeTerminalPathKindV1::TradingSuccess,
    RuntimeTerminalPathKindV1::ZeroFutureVolume,
    RuntimeTerminalPathKindV1::SourceFailure,
    RuntimeTerminalPathKindV1::ResolutionFailure,
];

/// Fail-closed refusal from the V1 runtime-liveness contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeLivenessErrorV1 {
    ZeroIdentity,
    IdentityAlias,
    DuplicateAccount,
    WrongPolicy,
    WrongLifecycle,
    WrongAccount,
    WrongOwner,
    WrongPayer,
    WrongNeutralSink,
    WrongCompartment,
    NoncanonicalCompartmentOrder,
    NoncanonicalTerminalPathOrder,
    ZeroMaximumCalls,
    ZeroMaximumCost,
    ZeroRentPrincipal,
    MissingMandatoryTerminalCall,
    TerminalPathExceedsMaximum,
    ArithmeticOverflow,
    FundingMismatch,
    BalanceShortfall,
    CallBudgetExhausted,
    CallCostExceedsMaximum,
    AlreadyClosed,
    InvalidPhase,
    InvalidFlags,
    ConservationFailure,
    CodecLength,
    CodecMagic,
    CodecVersion,
    CodecReserved,
    CodecEnum,
}

/// Result type for the V1 runtime-liveness contract.
pub type RuntimeLivenessResultV1<T> = Result<T, RuntimeLivenessErrorV1>;

fn live(id: Id) -> RuntimeLivenessResultV1<()> {
    if id.is_zero() {
        Err(RuntimeLivenessErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn add(left: u64, right: u64) -> RuntimeLivenessResultV1<u64> {
    left.checked_add(right)
        .ok_or(RuntimeLivenessErrorV1::ArithmeticOverflow)
}

fn multiply_u32_u64(left: u32, right: u64) -> RuntimeLivenessResultV1<u64> {
    u64::from(left)
        .checked_mul(right)
        .ok_or(RuntimeLivenessErrorV1::ArithmeticOverflow)
}

/// Semantic owner of one mandatory work reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeCompartmentKindV1 {
    Source = 0,
    Candidate = 1,
    Clearing = 2,
    Settlement = 3,
    Resolution = 4,
    Retirement = 5,
    Recovery = 6,
}

impl RuntimeCompartmentKindV1 {
    pub const fn index(self) -> usize {
        match self {
            Self::Source => 0,
            Self::Candidate => 1,
            Self::Clearing => 2,
            Self::Settlement => 3,
            Self::Resolution => 4,
            Self::Retirement => 5,
            Self::Recovery => 6,
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Source => 0,
            Self::Candidate => 1,
            Self::Clearing => 2,
            Self::Settlement => 3,
            Self::Resolution => 4,
            Self::Retirement => 5,
            Self::Recovery => 6,
        }
    }

    fn decode(value: u8) -> RuntimeLivenessResultV1<Self> {
        match value {
            0 => Ok(Self::Source),
            1 => Ok(Self::Candidate),
            2 => Ok(Self::Clearing),
            3 => Ok(Self::Settlement),
            4 => Ok(Self::Resolution),
            5 => Ok(Self::Retirement),
            6 => Ok(Self::Recovery),
            _ => Err(RuntimeLivenessErrorV1::CodecEnum),
        }
    }
}

/// Named finite lifecycle path whose calls must fit the prepaid maxima.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeTerminalPathKindV1 {
    TradingSuccess = 0,
    ZeroFutureVolume = 1,
    SourceFailure = 2,
    ResolutionFailure = 3,
}

impl RuntimeTerminalPathKindV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::TradingSuccess => 0,
            Self::ZeroFutureVolume => 1,
            Self::SourceFailure => 2,
            Self::ResolutionFailure => 3,
        }
    }

    fn decode(value: u8) -> RuntimeLivenessResultV1<Self> {
        match value {
            0 => Ok(Self::TradingSuccess),
            1 => Ok(Self::ZeroFutureVolume),
            2 => Ok(Self::SourceFailure),
            3 => Ok(Self::ResolutionFailure),
            _ => Err(RuntimeLivenessErrorV1::CodecEnum),
        }
    }
}

/// Immutable maximum calls, call payment, and refundable account rent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCompartmentPolicyV1 {
    pub kind: RuntimeCompartmentKindV1,
    pub maximum_calls: u32,
    pub maximum_lamports_per_call: u64,
    pub account_rent_principal_lamports: u64,
}

impl RuntimeCompartmentPolicyV1 {
    pub fn validate(self) -> RuntimeLivenessResultV1<()> {
        if self.maximum_calls == 0 {
            return Err(RuntimeLivenessErrorV1::ZeroMaximumCalls);
        }
        if self.maximum_lamports_per_call == 0 {
            return Err(RuntimeLivenessErrorV1::ZeroMaximumCost);
        }
        if self.account_rent_principal_lamports == 0 {
            return Err(RuntimeLivenessErrorV1::ZeroRentPrincipal);
        }
        self.work_capital_lamports()?;
        self.total_payer_debit_lamports()?;
        Ok(())
    }

    pub fn work_capital_lamports(self) -> RuntimeLivenessResultV1<u64> {
        multiply_u32_u64(self.maximum_calls, self.maximum_lamports_per_call)
    }

    pub fn total_payer_debit_lamports(self) -> RuntimeLivenessResultV1<u64> {
        add(
            self.work_capital_lamports()?,
            self.account_rent_principal_lamports,
        )
    }
}

/// Maximum calls consumed by one terminal path, in canonical compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTerminalPathV1 {
    pub kind: RuntimeTerminalPathKindV1,
    pub calls: [u32; RUNTIME_COMPARTMENT_COUNT_V1],
}

impl RuntimeTerminalPathV1 {
    pub const fn calls_for(self, kind: RuntimeCompartmentKindV1) -> u32 {
        self.calls[kind.index()]
    }
}

/// Immutable, complete, zero-future-volume-capitalized liveness policy.
///
/// The only monetary units are native lamports already present at admission.
/// The type has no route for Hoard principal, collateral, future fees, future
/// subscribers, or projected volume to satisfy an obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLivenessPolicyV1 {
    pub policy_id: Id,
    pub realm_id: Id,
    pub neutral_sink: Id,
    pub compartments: [RuntimeCompartmentPolicyV1; RUNTIME_COMPARTMENT_COUNT_V1],
    pub terminal_paths: [RuntimeTerminalPathV1; RUNTIME_TERMINAL_PATH_COUNT_V1],
    pub flags: u16,
}

impl RuntimeLivenessPolicyV1 {
    pub fn validate(self) -> RuntimeLivenessResultV1<()> {
        live(self.policy_id)?;
        live(self.realm_id)?;
        live(self.neutral_sink)?;
        if self.flags != 0 {
            return Err(RuntimeLivenessErrorV1::InvalidFlags);
        }
        let mut compartment_index = 0usize;
        while compartment_index < RUNTIME_COMPARTMENT_COUNT_V1 {
            let compartment = self.compartments[compartment_index];
            if compartment.kind != RUNTIME_COMPARTMENT_ORDER_V1[compartment_index] {
                return Err(RuntimeLivenessErrorV1::NoncanonicalCompartmentOrder);
            }
            compartment.validate()?;
            compartment_index += 1;
        }
        let mut path_index = 0usize;
        while path_index < RUNTIME_TERMINAL_PATH_COUNT_V1 {
            let path = self.terminal_paths[path_index];
            if path.kind != RUNTIME_TERMINAL_PATH_ORDER_V1[path_index] {
                return Err(RuntimeLivenessErrorV1::NoncanonicalTerminalPathOrder);
            }
            let mut index = 0usize;
            while index < RUNTIME_COMPARTMENT_COUNT_V1 {
                if path.calls[index] > self.compartments[index].maximum_calls {
                    return Err(RuntimeLivenessErrorV1::TerminalPathExceedsMaximum);
                }
                index += 1;
            }
            path_index += 1;
        }
        self.validate_mandatory_paths()
    }

    fn validate_mandatory_paths(self) -> RuntimeLivenessResultV1<()> {
        let trading = self.terminal_paths[0];
        for kind in [
            RuntimeCompartmentKindV1::Source,
            RuntimeCompartmentKindV1::Candidate,
            RuntimeCompartmentKindV1::Clearing,
            RuntimeCompartmentKindV1::Settlement,
            RuntimeCompartmentKindV1::Resolution,
            RuntimeCompartmentKindV1::Retirement,
        ] {
            if trading.calls_for(kind) == 0 {
                return Err(RuntimeLivenessErrorV1::MissingMandatoryTerminalCall);
            }
        }

        let zero_volume = self.terminal_paths[1];
        for kind in [
            RuntimeCompartmentKindV1::Source,
            RuntimeCompartmentKindV1::Candidate,
            RuntimeCompartmentKindV1::Clearing,
            RuntimeCompartmentKindV1::Resolution,
            RuntimeCompartmentKindV1::Retirement,
            RuntimeCompartmentKindV1::Recovery,
        ] {
            if zero_volume.calls_for(kind) == 0 {
                return Err(RuntimeLivenessErrorV1::MissingMandatoryTerminalCall);
            }
        }

        let source_failure = self.terminal_paths[2];
        for kind in [
            RuntimeCompartmentKindV1::Source,
            RuntimeCompartmentKindV1::Retirement,
            RuntimeCompartmentKindV1::Recovery,
        ] {
            if source_failure.calls_for(kind) == 0 {
                return Err(RuntimeLivenessErrorV1::MissingMandatoryTerminalCall);
            }
        }

        let resolution_failure = self.terminal_paths[3];
        for kind in [
            RuntimeCompartmentKindV1::Source,
            RuntimeCompartmentKindV1::Candidate,
            RuntimeCompartmentKindV1::Clearing,
            RuntimeCompartmentKindV1::Resolution,
            RuntimeCompartmentKindV1::Retirement,
            RuntimeCompartmentKindV1::Recovery,
        ] {
            if resolution_failure.calls_for(kind) == 0 {
                return Err(RuntimeLivenessErrorV1::MissingMandatoryTerminalCall);
            }
        }
        Ok(())
    }

    pub const fn compartment(
        self,
        kind: RuntimeCompartmentKindV1,
    ) -> RuntimeCompartmentPolicyV1 {
        self.compartments[kind.index()]
    }

    /// Exact admission debit across all seven payer-funded accounts.
    pub fn total_payer_debit_lamports(self) -> RuntimeLivenessResultV1<u64> {
        self.validate()?;
        let mut total = 0u64;
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            total = add(
                total,
                self.compartments[index].total_payer_debit_lamports()?,
            )?;
            index += 1;
        }
        Ok(total)
    }

    pub fn encode(self, output: &mut [u8]) -> RuntimeLivenessResultV1<()> {
        self.validate()?;
        let mut writer = Writer::exact(output, RUNTIME_LIVENESS_POLICY_BYTES_V1)?;
        writer.array(RUNTIME_LIVENESS_POLICY_MAGIC_V1)?;
        writer.u16(RUNTIME_LIVENESS_VERSION_V1)?;
        writer.u16(self.flags)?;
        writer.id(self.policy_id)?;
        writer.id(self.realm_id)?;
        writer.id(self.neutral_sink)?;
        for compartment in self.compartments {
            writer.u8(compartment.kind.byte())?;
            writer.reserved(3)?;
            writer.u32(compartment.maximum_calls)?;
            writer.u64(compartment.maximum_lamports_per_call)?;
            writer.u64(compartment.account_rent_principal_lamports)?;
        }
        for path in self.terminal_paths {
            writer.u8(path.kind.byte())?;
            writer.reserved(3)?;
            for calls in path.calls {
                writer.u32(calls)?;
            }
        }
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> RuntimeLivenessResultV1<Self> {
        let mut reader = Reader::exact(input, RUNTIME_LIVENESS_POLICY_BYTES_V1)?;
        if reader.array::<8>()? != RUNTIME_LIVENESS_POLICY_MAGIC_V1 {
            return Err(RuntimeLivenessErrorV1::CodecMagic);
        }
        if reader.u16()? != RUNTIME_LIVENESS_VERSION_V1 {
            return Err(RuntimeLivenessErrorV1::CodecVersion);
        }
        let flags = reader.u16()?;
        let policy_id = reader.id()?;
        let realm_id = reader.id()?;
        let neutral_sink = reader.id()?;
        let mut compartments = [RuntimeCompartmentPolicyV1 {
            kind: RuntimeCompartmentKindV1::Source,
            maximum_calls: 0,
            maximum_lamports_per_call: 0,
            account_rent_principal_lamports: 0,
        }; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            compartments[index] = RuntimeCompartmentPolicyV1 {
                kind: RuntimeCompartmentKindV1::decode(reader.u8()?)?,
                maximum_calls: {
                    reader.reserved(3)?;
                    reader.u32()?
                },
                maximum_lamports_per_call: reader.u64()?,
                account_rent_principal_lamports: reader.u64()?,
            };
            index += 1;
        }
        let empty_path = RuntimeTerminalPathV1 {
            kind: RuntimeTerminalPathKindV1::TradingSuccess,
            calls: [0; RUNTIME_COMPARTMENT_COUNT_V1],
        };
        let mut terminal_paths = [empty_path; RUNTIME_TERMINAL_PATH_COUNT_V1];
        index = 0;
        while index < RUNTIME_TERMINAL_PATH_COUNT_V1 {
            let kind = RuntimeTerminalPathKindV1::decode(reader.u8()?)?;
            reader.reserved(3)?;
            let mut calls = [0u32; RUNTIME_COMPARTMENT_COUNT_V1];
            let mut call_index = 0usize;
            while call_index < RUNTIME_COMPARTMENT_COUNT_V1 {
                calls[call_index] = reader.u32()?;
                call_index += 1;
            }
            terminal_paths[index] = RuntimeTerminalPathV1 { kind, calls };
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            policy_id,
            realm_id,
            neutral_sink,
            compartments,
            terminal_paths,
            flags,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Immutable identity fields persisted in each compartment account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCompartmentIdentityV1 {
    pub policy_id: Id,
    pub lifecycle_id: Id,
    pub account_id: Id,
    pub owner: Id,
    pub payer: Id,
    pub neutral_sink: Id,
    pub generation: u64,
}

impl RuntimeCompartmentIdentityV1 {
    pub fn validate(self) -> RuntimeLivenessResultV1<()> {
        for identity in [
            self.policy_id,
            self.lifecycle_id,
            self.account_id,
            self.owner,
            self.payer,
            self.neutral_sink,
        ] {
            live(identity)?;
        }
        if self.neutral_sink == self.account_id
            || self.neutral_sink == self.owner
            || self.neutral_sink == self.payer
            || self.account_id == self.owner
            || self.account_id == self.payer
        {
            return Err(RuntimeLivenessErrorV1::IdentityAlias);
        }
        Ok(())
    }
}

/// Adapter-attested present native-lamport funding for one account.
///
/// The payer debit and account delta must both equal work plus rent. Existing
/// account balance is never credited to the payer: it becomes sink-owned
/// donation. The adapter must authenticate the payer debit and atomic transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentFundingV1 {
    pub payer: Id,
    pub source: PresentFundingSourceV1,
    pub payer_debit_lamports: u64,
    pub account_balance_before: u64,
    pub account_balance_after: u64,
}

/// Admissible present-balance origins.
///
/// There is intentionally no Hoard, collateral, fee-vault, projected-fee, or
/// future-volume variant. The adapter authenticates the selected class and the
/// debit from `payer`; inventing another source is a hard ABI refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PresentFundingSourceV1 {
    ExternalSignerNativeLamports = 0,
    PrecapitalizedLivenessEndowment = 1,
}

impl PresentFundingSourceV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::ExternalSignerNativeLamports => 0,
            Self::PrecapitalizedLivenessEndowment => 1,
        }
    }

    fn decode(value: u8) -> RuntimeLivenessResultV1<Self> {
        match value {
            0 => Ok(Self::ExternalSignerNativeLamports),
            1 => Ok(Self::PrecapitalizedLivenessEndowment),
            _ => Err(RuntimeLivenessErrorV1::CodecEnum),
        }
    }
}

/// Admission request for one canonical compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCompartmentAdmissionV1 {
    pub kind: RuntimeCompartmentKindV1,
    pub identity: RuntimeCompartmentIdentityV1,
    pub funding: PresentFundingV1,
}

/// Runtime phase. Closed accounts carry disposition evidence but no balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeCompartmentPhaseV1 {
    Active = 0,
    ClosedSuccess = 1,
    ClosedFailure = 2,
}

impl RuntimeCompartmentPhaseV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::ClosedSuccess => 1,
            Self::ClosedFailure => 2,
        }
    }

    fn decode(value: u8) -> RuntimeLivenessResultV1<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::ClosedSuccess),
            2 => Ok(Self::ClosedFailure),
            _ => Err(RuntimeLivenessErrorV1::CodecEnum),
        }
    }
}

/// Exact movement emitted for one successfully completed bounded call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCallMovementV1 {
    pub kind: RuntimeCompartmentKindV1,
    pub account: Id,
    pub keeper: Id,
    pub keeper_lamports: u64,
    pub payer: Id,
    pub payer_refund_lamports: u64,
}

/// Adapter-observed account balance before and after one atomic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBalanceTransitionV1 {
    pub account_balance_before: u64,
    pub account_balance_after: u64,
}

/// Exact terminal movement emitted when the account becomes deletable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTerminalMovementV1 {
    pub kind: RuntimeCompartmentKindV1,
    pub account: Id,
    pub payer: Id,
    pub payer_refund_lamports: u64,
    pub neutral_sink: Id,
    pub neutral_lamports: u64,
    pub success: bool,
}

/// Persisted exact accounting for one mandatory runtime compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCompartmentV1 {
    pub kind: RuntimeCompartmentKindV1,
    pub phase: RuntimeCompartmentPhaseV1,
    pub funding_source: PresentFundingSourceV1,
    pub identity: RuntimeCompartmentIdentityV1,
    pub maximum_calls: u32,
    pub remaining_calls: u32,
    pub maximum_lamports_per_call: u64,
    pub capitalized_work_lamports: u64,
    pub remaining_work_lamports: u64,
    pub keeper_paid_lamports: u64,
    pub payer_refunded_work_lamports: u64,
    pub neutral_sinked_work_lamports: u64,
    pub rent_principal_lamports: u64,
    pub rent_locked_lamports: u64,
    pub rent_refunded_lamports: u64,
    pub donation_received_lamports: u64,
    pub donation_remaining_lamports: u64,
    pub donation_sinked_lamports: u64,
    pub flags: u16,
}

impl RuntimeCompartmentV1 {
    pub fn admit(
        policy: RuntimeLivenessPolicyV1,
        admission: RuntimeCompartmentAdmissionV1,
    ) -> RuntimeLivenessResultV1<Self> {
        policy.validate()?;
        admission.identity.validate()?;
        if admission.identity.policy_id != policy.policy_id {
            return Err(RuntimeLivenessErrorV1::WrongPolicy);
        }
        if admission.identity.neutral_sink != policy.neutral_sink {
            return Err(RuntimeLivenessErrorV1::WrongNeutralSink);
        }
        if admission.funding.payer != admission.identity.payer {
            return Err(RuntimeLivenessErrorV1::WrongPayer);
        }
        let compartment_policy = policy.compartment(admission.kind);
        if compartment_policy.kind != admission.kind {
            return Err(RuntimeLivenessErrorV1::WrongCompartment);
        }
        let required = compartment_policy.total_payer_debit_lamports()?;
        if admission.funding.payer_debit_lamports != required
            || add(admission.funding.account_balance_before, required)?
                != admission.funding.account_balance_after
        {
            return Err(RuntimeLivenessErrorV1::FundingMismatch);
        }
        let value = Self {
            kind: admission.kind,
            phase: RuntimeCompartmentPhaseV1::Active,
            funding_source: admission.funding.source,
            identity: admission.identity,
            maximum_calls: compartment_policy.maximum_calls,
            remaining_calls: compartment_policy.maximum_calls,
            maximum_lamports_per_call: compartment_policy.maximum_lamports_per_call,
            capitalized_work_lamports: compartment_policy.work_capital_lamports()?,
            remaining_work_lamports: compartment_policy.work_capital_lamports()?,
            keeper_paid_lamports: 0,
            payer_refunded_work_lamports: 0,
            neutral_sinked_work_lamports: 0,
            rent_principal_lamports: compartment_policy.account_rent_principal_lamports,
            rent_locked_lamports: compartment_policy.account_rent_principal_lamports,
            rent_refunded_lamports: 0,
            donation_received_lamports: admission.funding.account_balance_before,
            donation_remaining_lamports: admission.funding.account_balance_before,
            donation_sinked_lamports: 0,
            flags: 0,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> RuntimeLivenessResultV1<()> {
        self.identity.validate()?;
        if self.flags != 0 {
            return Err(RuntimeLivenessErrorV1::InvalidFlags);
        }
        if self.maximum_calls == 0 {
            return Err(RuntimeLivenessErrorV1::ZeroMaximumCalls);
        }
        if self.maximum_lamports_per_call == 0 {
            return Err(RuntimeLivenessErrorV1::ZeroMaximumCost);
        }
        if self.rent_principal_lamports == 0 {
            return Err(RuntimeLivenessErrorV1::ZeroRentPrincipal);
        }
        if self.remaining_calls > self.maximum_calls
            || self.remaining_work_lamports
                != multiply_u32_u64(self.remaining_calls, self.maximum_lamports_per_call)?
        {
            return Err(RuntimeLivenessErrorV1::ConservationFailure);
        }
        let work_accounted = add(
            add(
                self.remaining_work_lamports,
                self.keeper_paid_lamports,
            )?,
            add(
                self.payer_refunded_work_lamports,
                self.neutral_sinked_work_lamports,
            )?,
        )?;
        if work_accounted != self.capitalized_work_lamports {
            return Err(RuntimeLivenessErrorV1::ConservationFailure);
        }
        if add(self.rent_locked_lamports, self.rent_refunded_lamports)?
            != self.rent_principal_lamports
            || add(
                self.donation_remaining_lamports,
                self.donation_sinked_lamports,
            )? != self.donation_received_lamports
        {
            return Err(RuntimeLivenessErrorV1::ConservationFailure);
        }
        match self.phase {
            RuntimeCompartmentPhaseV1::Active => {
                if self.rent_locked_lamports != self.rent_principal_lamports
                    || self.rent_refunded_lamports != 0
                    || self.neutral_sinked_work_lamports != 0
                    || self.donation_sinked_lamports != 0
                {
                    return Err(RuntimeLivenessErrorV1::InvalidPhase);
                }
            }
            RuntimeCompartmentPhaseV1::ClosedSuccess => {
                if self.remaining_calls != 0
                    || self.remaining_work_lamports != 0
                    || self.neutral_sinked_work_lamports != 0
                    || self.rent_locked_lamports != 0
                    || self.rent_refunded_lamports != self.rent_principal_lamports
                    || self.donation_remaining_lamports != 0
                    || self.donation_sinked_lamports != self.donation_received_lamports
                {
                    return Err(RuntimeLivenessErrorV1::InvalidPhase);
                }
            }
            RuntimeCompartmentPhaseV1::ClosedFailure => {
                if self.remaining_calls != 0
                    || self.remaining_work_lamports != 0
                    || self.rent_locked_lamports != 0
                    || self.rent_refunded_lamports != self.rent_principal_lamports
                    || self.donation_remaining_lamports != 0
                    || self.donation_sinked_lamports != self.donation_received_lamports
                {
                    return Err(RuntimeLivenessErrorV1::InvalidPhase);
                }
                let completed_reservations = add(
                    self.keeper_paid_lamports,
                    self.payer_refunded_work_lamports,
                )?;
                if completed_reservations % self.maximum_lamports_per_call != 0 {
                    return Err(RuntimeLivenessErrorV1::ConservationFailure);
                }
            }
        }
        Ok(())
    }

    /// Lamports that must remain in the physical account after observing all
    /// surplus: unspent work, locked rent, and sink-owned donation.
    pub fn expected_account_balance_lamports(self) -> RuntimeLivenessResultV1<u64> {
        self.validate()?;
        add(
            self.remaining_work_lamports,
            add(
                self.rent_locked_lamports,
                self.donation_remaining_lamports,
            )?,
        )
    }

    /// Absorb newly observed surplus as donation without crediting work/rent.
    pub fn observe_balance(mut self, actual_balance: u64) -> RuntimeLivenessResultV1<Self> {
        self.ensure_active()?;
        let expected = self.expected_account_balance_lamports()?;
        if actual_balance < expected {
            return Err(RuntimeLivenessErrorV1::BalanceShortfall);
        }
        let new_donation = actual_balance - expected;
        self.donation_received_lamports = add(self.donation_received_lamports, new_donation)?;
        self.donation_remaining_lamports = add(self.donation_remaining_lamports, new_donation)?;
        self.validate()?;
        Ok(self)
    }

    fn ensure_active(self) -> RuntimeLivenessResultV1<()> {
        if self.phase != RuntimeCompartmentPhaseV1::Active {
            return Err(RuntimeLivenessErrorV1::AlreadyClosed);
        }
        Ok(())
    }

    /// Consume one bounded mandatory call, pay its actual accepted cost, and
    /// refund the unused per-call headroom to the immutable payer immediately.
    pub fn spend_call(
        mut self,
        owner: Id,
        keeper: Id,
        keeper_payment_lamports: u64,
        balances: RuntimeBalanceTransitionV1,
    ) -> RuntimeLivenessResultV1<(Self, RuntimeCallMovementV1)> {
        self.ensure_active()?;
        self.validate()?;
        live(keeper)?;
        if owner != self.identity.owner {
            return Err(RuntimeLivenessErrorV1::WrongOwner);
        }
        if keeper == self.identity.account_id || keeper == self.identity.neutral_sink {
            return Err(RuntimeLivenessErrorV1::IdentityAlias);
        }
        if self.remaining_calls == 0 {
            return Err(RuntimeLivenessErrorV1::CallBudgetExhausted);
        }
        if keeper_payment_lamports > self.maximum_lamports_per_call {
            return Err(RuntimeLivenessErrorV1::CallCostExceedsMaximum);
        }
        self = self.observe_balance(balances.account_balance_before)?;
        if add(
            balances.account_balance_after,
            self.maximum_lamports_per_call,
        )? != balances.account_balance_before
        {
            return Err(RuntimeLivenessErrorV1::FundingMismatch);
        }
        let refund = self.maximum_lamports_per_call - keeper_payment_lamports;
        self.remaining_calls -= 1;
        self.remaining_work_lamports -= self.maximum_lamports_per_call;
        self.keeper_paid_lamports = add(self.keeper_paid_lamports, keeper_payment_lamports)?;
        self.payer_refunded_work_lamports =
            add(self.payer_refunded_work_lamports, refund)?;
        self.validate()?;
        Ok((
            self,
            RuntimeCallMovementV1 {
                kind: self.kind,
                account: self.identity.account_id,
                keeper,
                keeper_lamports: keeper_payment_lamports,
                payer: self.identity.payer,
                payer_refund_lamports: refund,
            },
        ))
    }

    /// Close after adapter-authenticated successful terminality. Unused work
    /// and rent return to the payer; every donation goes to the neutral sink.
    pub fn close_success(
        mut self,
        owner: Id,
        balances: RuntimeBalanceTransitionV1,
    ) -> RuntimeLivenessResultV1<(Self, RuntimeTerminalMovementV1)> {
        self.ensure_active()?;
        self.validate()?;
        if owner != self.identity.owner {
            return Err(RuntimeLivenessErrorV1::WrongOwner);
        }
        self = self.observe_balance(balances.account_balance_before)?;
        if balances.account_balance_after != 0
            || self.expected_account_balance_lamports()? != balances.account_balance_before
        {
            return Err(RuntimeLivenessErrorV1::FundingMismatch);
        }
        let payer_refund_lamports = add(
            self.remaining_work_lamports,
            self.rent_locked_lamports,
        )?;
        self.payer_refunded_work_lamports = add(
            self.payer_refunded_work_lamports,
            self.remaining_work_lamports,
        )?;
        self.rent_refunded_lamports = self.rent_locked_lamports;
        self.donation_sinked_lamports = self.donation_remaining_lamports;
        self.remaining_calls = 0;
        self.remaining_work_lamports = 0;
        self.rent_locked_lamports = 0;
        self.donation_remaining_lamports = 0;
        self.phase = RuntimeCompartmentPhaseV1::ClosedSuccess;
        self.validate()?;
        Ok((
            self,
            RuntimeTerminalMovementV1 {
                kind: self.kind,
                account: self.identity.account_id,
                payer: self.identity.payer,
                payer_refund_lamports,
                neutral_sink: self.identity.neutral_sink,
                neutral_lamports: self.donation_sinked_lamports,
                success: true,
            },
        ))
    }

    /// Close after adapter-authenticated irrecoverable terminal failure. Work
    /// residue and donations go to the neutral sink; rent alone returns to the
    /// payer, so interested parties cannot profit from declaring failure.
    pub fn close_failure(
        mut self,
        owner: Id,
        balances: RuntimeBalanceTransitionV1,
    ) -> RuntimeLivenessResultV1<(Self, RuntimeTerminalMovementV1)> {
        self.ensure_active()?;
        self.validate()?;
        if owner != self.identity.owner {
            return Err(RuntimeLivenessErrorV1::WrongOwner);
        }
        self = self.observe_balance(balances.account_balance_before)?;
        if balances.account_balance_after != 0
            || self.expected_account_balance_lamports()? != balances.account_balance_before
        {
            return Err(RuntimeLivenessErrorV1::FundingMismatch);
        }
        let neutral_lamports = add(
            self.remaining_work_lamports,
            self.donation_remaining_lamports,
        )?;
        self.neutral_sinked_work_lamports = self.remaining_work_lamports;
        self.rent_refunded_lamports = self.rent_locked_lamports;
        self.donation_sinked_lamports = self.donation_remaining_lamports;
        self.remaining_calls = 0;
        self.remaining_work_lamports = 0;
        self.rent_locked_lamports = 0;
        self.donation_remaining_lamports = 0;
        self.phase = RuntimeCompartmentPhaseV1::ClosedFailure;
        self.validate()?;
        Ok((
            self,
            RuntimeTerminalMovementV1 {
                kind: self.kind,
                account: self.identity.account_id,
                payer: self.identity.payer,
                payer_refund_lamports: self.rent_refunded_lamports,
                neutral_sink: self.identity.neutral_sink,
                neutral_lamports,
                success: false,
            },
        ))
    }

    pub fn validate_against_policy(
        self,
        policy: RuntimeLivenessPolicyV1,
    ) -> RuntimeLivenessResultV1<()> {
        self.validate()?;
        policy.validate()?;
        if self.identity.policy_id != policy.policy_id {
            return Err(RuntimeLivenessErrorV1::WrongPolicy);
        }
        if self.identity.neutral_sink != policy.neutral_sink {
            return Err(RuntimeLivenessErrorV1::WrongNeutralSink);
        }
        let expected = policy.compartment(self.kind);
        if self.maximum_calls != expected.maximum_calls
            || self.maximum_lamports_per_call != expected.maximum_lamports_per_call
            || self.rent_principal_lamports != expected.account_rent_principal_lamports
        {
            return Err(RuntimeLivenessErrorV1::WrongPolicy);
        }
        Ok(())
    }

    pub fn encode(self, output: &mut [u8]) -> RuntimeLivenessResultV1<()> {
        self.validate()?;
        let mut writer = Writer::exact(output, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
        writer.array(RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1)?;
        writer.u16(RUNTIME_LIVENESS_VERSION_V1)?;
        writer.u16(self.flags)?;
        writer.u8(self.kind.byte())?;
        writer.u8(self.phase.byte())?;
        writer.u8(self.funding_source.byte())?;
        writer.reserved(1)?;
        writer.id(self.identity.policy_id)?;
        writer.id(self.identity.lifecycle_id)?;
        writer.id(self.identity.account_id)?;
        writer.id(self.identity.owner)?;
        writer.id(self.identity.payer)?;
        writer.id(self.identity.neutral_sink)?;
        writer.u64(self.identity.generation)?;
        writer.u32(self.maximum_calls)?;
        writer.u32(self.remaining_calls)?;
        for value in [
            self.maximum_lamports_per_call,
            self.capitalized_work_lamports,
            self.remaining_work_lamports,
            self.keeper_paid_lamports,
            self.payer_refunded_work_lamports,
            self.neutral_sinked_work_lamports,
            self.rent_principal_lamports,
            self.rent_locked_lamports,
            self.rent_refunded_lamports,
            self.donation_received_lamports,
            self.donation_remaining_lamports,
            self.donation_sinked_lamports,
        ] {
            writer.u64(value)?;
        }
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> RuntimeLivenessResultV1<Self> {
        let mut reader = Reader::exact(input, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
        if reader.array::<8>()? != RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1 {
            return Err(RuntimeLivenessErrorV1::CodecMagic);
        }
        if reader.u16()? != RUNTIME_LIVENESS_VERSION_V1 {
            return Err(RuntimeLivenessErrorV1::CodecVersion);
        }
        let flags = reader.u16()?;
        let kind = RuntimeCompartmentKindV1::decode(reader.u8()?)?;
        let phase = RuntimeCompartmentPhaseV1::decode(reader.u8()?)?;
        let funding_source = PresentFundingSourceV1::decode(reader.u8()?)?;
        reader.reserved(1)?;
        let identity = RuntimeCompartmentIdentityV1 {
            policy_id: reader.id()?,
            lifecycle_id: reader.id()?,
            account_id: reader.id()?,
            owner: reader.id()?,
            payer: reader.id()?,
            neutral_sink: reader.id()?,
            generation: reader.u64()?,
        };
        let maximum_calls = reader.u32()?;
        let remaining_calls = reader.u32()?;
        let value = Self {
            kind,
            phase,
            funding_source,
            identity,
            maximum_calls,
            remaining_calls,
            maximum_lamports_per_call: reader.u64()?,
            capitalized_work_lamports: reader.u64()?,
            remaining_work_lamports: reader.u64()?,
            keeper_paid_lamports: reader.u64()?,
            payer_refunded_work_lamports: reader.u64()?,
            neutral_sinked_work_lamports: reader.u64()?,
            rent_principal_lamports: reader.u64()?,
            rent_locked_lamports: reader.u64()?,
            rent_refunded_lamports: reader.u64()?,
            donation_received_lamports: reader.u64()?,
            donation_remaining_lamports: reader.u64()?,
            donation_sinked_lamports: reader.u64()?,
            flags,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Complete seven-account admission result for one lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLivenessBundleV1 {
    pub policy_id: Id,
    pub lifecycle_id: Id,
    pub compartments: [RuntimeCompartmentV1; RUNTIME_COMPARTMENT_COUNT_V1],
}

/// Adapter-side immutable join for a separately versioned lifecycle account.
///
/// This projection does not require another protocol family to invent a new
/// persisted field. Its caller compares authenticated policy/lifecycle roots
/// and the seven canonical child identities before composing transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLivenessBundleBindingV1 {
    pub policy_id: Id,
    pub realm_id: Id,
    pub lifecycle_id: Id,
    pub neutral_sink: Id,
    pub account_ids: [Id; RUNTIME_COMPARTMENT_COUNT_V1],
    pub owners: [Id; RUNTIME_COMPARTMENT_COUNT_V1],
    pub payers: [Id; RUNTIME_COMPARTMENT_COUNT_V1],
    pub generations: [u64; RUNTIME_COMPARTMENT_COUNT_V1],
}

impl RuntimeLivenessBundleV1 {
    /// Atomically construct the pure bundle only when all seven compartments
    /// are canonical, distinct, and presently funded. An adapter must preserve
    /// this all-or-nothing relation across account creation and transfers.
    pub fn admit(
        policy: RuntimeLivenessPolicyV1,
        lifecycle_id: Id,
        admissions: [RuntimeCompartmentAdmissionV1; RUNTIME_COMPARTMENT_COUNT_V1],
    ) -> RuntimeLivenessResultV1<Self> {
        policy.validate()?;
        live(lifecycle_id)?;
        let first = RuntimeCompartmentV1::admit(policy, admissions[0])?;
        let mut compartments = [first; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            if admissions[index].kind != RUNTIME_COMPARTMENT_ORDER_V1[index] {
                return Err(RuntimeLivenessErrorV1::NoncanonicalCompartmentOrder);
            }
            if admissions[index].identity.lifecycle_id != lifecycle_id {
                return Err(RuntimeLivenessErrorV1::WrongLifecycle);
            }
            let mut prior = 0usize;
            while prior < index {
                if admissions[prior].identity.account_id == admissions[index].identity.account_id {
                    return Err(RuntimeLivenessErrorV1::DuplicateAccount);
                }
                prior += 1;
            }
            compartments[index] = RuntimeCompartmentV1::admit(policy, admissions[index])?;
            index += 1;
        }
        let bundle = Self {
            policy_id: policy.policy_id,
            lifecycle_id,
            compartments,
        };
        bundle.validate(policy)?;
        Ok(bundle)
    }

    pub fn validate(self, policy: RuntimeLivenessPolicyV1) -> RuntimeLivenessResultV1<()> {
        policy.validate()?;
        live(self.lifecycle_id)?;
        if self.policy_id != policy.policy_id {
            return Err(RuntimeLivenessErrorV1::WrongPolicy);
        }
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            let compartment = self.compartments[index];
            if compartment.kind != RUNTIME_COMPARTMENT_ORDER_V1[index] {
                return Err(RuntimeLivenessErrorV1::NoncanonicalCompartmentOrder);
            }
            if compartment.identity.lifecycle_id != self.lifecycle_id {
                return Err(RuntimeLivenessErrorV1::WrongLifecycle);
            }
            compartment.validate_against_policy(policy)?;
            let mut prior = 0usize;
            while prior < index {
                if self.compartments[prior].identity.account_id == compartment.identity.account_id {
                    return Err(RuntimeLivenessErrorV1::DuplicateAccount);
                }
                prior += 1;
            }
            index += 1;
        }
        Ok(())
    }

    pub const fn compartment(
        self,
        kind: RuntimeCompartmentKindV1,
    ) -> RuntimeCompartmentV1 {
        self.compartments[kind.index()]
    }

    pub fn replace_compartment(
        mut self,
        policy: RuntimeLivenessPolicyV1,
        compartment: RuntimeCompartmentV1,
    ) -> RuntimeLivenessResultV1<Self> {
        self.validate(policy)?;
        compartment.validate_against_policy(policy)?;
        if compartment.identity.lifecycle_id != self.lifecycle_id {
            return Err(RuntimeLivenessErrorV1::WrongLifecycle);
        }
        let index = compartment.kind.index();
        if compartment.identity.account_id != self.compartments[index].identity.account_id
            || compartment.identity.owner != self.compartments[index].identity.owner
            || compartment.identity.payer != self.compartments[index].identity.payer
            || compartment.identity.generation != self.compartments[index].identity.generation
            || compartment.funding_source != self.compartments[index].funding_source
        {
            return Err(RuntimeLivenessErrorV1::WrongAccount);
        }
        self.compartments[index] = compartment;
        self.validate(policy)?;
        Ok(self)
    }

    /// Exact live balance across all seven physical accounts.
    pub fn expected_live_balance_lamports(self) -> RuntimeLivenessResultV1<u64> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            total = add(
                total,
                self.compartments[index].expected_account_balance_lamports()?,
            )?;
            index += 1;
        }
        Ok(total)
    }

    pub fn binding(
        self,
        policy: RuntimeLivenessPolicyV1,
    ) -> RuntimeLivenessResultV1<RuntimeLivenessBundleBindingV1> {
        self.validate(policy)?;
        let mut account_ids = [Id::ZERO; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut owners = [Id::ZERO; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut payers = [Id::ZERO; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut generations = [0u64; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            let identity = self.compartments[index].identity;
            account_ids[index] = identity.account_id;
            owners[index] = identity.owner;
            payers[index] = identity.payer;
            generations[index] = identity.generation;
            index += 1;
        }
        Ok(RuntimeLivenessBundleBindingV1 {
            policy_id: self.policy_id,
            realm_id: policy.realm_id,
            lifecycle_id: self.lifecycle_id,
            neutral_sink: policy.neutral_sink,
            account_ids,
            owners,
            payers,
            generations,
        })
    }

    pub fn all_closed(self) -> RuntimeLivenessResultV1<bool> {
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            self.compartments[index].validate()?;
            if self.compartments[index].phase == RuntimeCompartmentPhaseV1::Active {
                return Ok(false);
            }
            index += 1;
        }
        Ok(true)
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    cursor: usize,
}

impl<'a> Writer<'a> {
    fn exact(output: &'a mut [u8], expected: usize) -> RuntimeLivenessResultV1<Self> {
        if output.len() != expected {
            return Err(RuntimeLivenessErrorV1::CodecLength);
        }
        output.fill(0);
        Ok(Self { output, cursor: 0 })
    }

    fn bytes(&mut self, value: &[u8]) -> RuntimeLivenessResultV1<()> {
        let end = self
            .cursor
            .checked_add(value.len())
            .ok_or(RuntimeLivenessErrorV1::ArithmeticOverflow)?;
        let destination = self
            .output
            .get_mut(self.cursor..end)
            .ok_or(RuntimeLivenessErrorV1::CodecLength)?;
        destination.copy_from_slice(value);
        self.cursor = end;
        Ok(())
    }

    fn array<const N: usize>(&mut self, value: [u8; N]) -> RuntimeLivenessResultV1<()> {
        self.bytes(&value)
    }

    fn id(&mut self, value: Id) -> RuntimeLivenessResultV1<()> {
        self.array(value.bytes())
    }

    fn u8(&mut self, value: u8) -> RuntimeLivenessResultV1<()> {
        self.array([value])
    }

    fn u16(&mut self, value: u16) -> RuntimeLivenessResultV1<()> {
        self.array(value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> RuntimeLivenessResultV1<()> {
        self.array(value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> RuntimeLivenessResultV1<()> {
        self.array(value.to_le_bytes())
    }

    fn reserved(&mut self, count: usize) -> RuntimeLivenessResultV1<()> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RuntimeLivenessErrorV1::ArithmeticOverflow)?;
        if self.output.get(self.cursor..end).is_none() {
            return Err(RuntimeLivenessErrorV1::CodecLength);
        }
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> RuntimeLivenessResultV1<()> {
        if self.cursor != self.output.len() {
            return Err(RuntimeLivenessErrorV1::CodecLength);
        }
        Ok(())
    }
}

struct Reader<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn exact(input: &'a [u8], expected: usize) -> RuntimeLivenessResultV1<Self> {
        if input.len() != expected {
            return Err(RuntimeLivenessErrorV1::CodecLength);
        }
        Ok(Self { input, cursor: 0 })
    }

    fn array<const N: usize>(&mut self) -> RuntimeLivenessResultV1<[u8; N]> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(RuntimeLivenessErrorV1::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.cursor..end)
            .ok_or(RuntimeLivenessErrorV1::CodecLength)?;
        let mut output = [0u8; N];
        output.copy_from_slice(source);
        self.cursor = end;
        Ok(output)
    }

    fn id(&mut self) -> RuntimeLivenessResultV1<Id> {
        Ok(Id::from_bytes(self.array()?))
    }

    fn u8(&mut self) -> RuntimeLivenessResultV1<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> RuntimeLivenessResultV1<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> RuntimeLivenessResultV1<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> RuntimeLivenessResultV1<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn reserved(&mut self, count: usize) -> RuntimeLivenessResultV1<()> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RuntimeLivenessErrorV1::ArithmeticOverflow)?;
        let reserved = self
            .input
            .get(self.cursor..end)
            .ok_or(RuntimeLivenessErrorV1::CodecLength)?;
        if reserved.iter().any(|byte| *byte != 0) {
            return Err(RuntimeLivenessErrorV1::CodecReserved);
        }
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> RuntimeLivenessResultV1<()> {
        if self.cursor != self.input.len() {
            return Err(RuntimeLivenessErrorV1::CodecLength);
        }
        Ok(())
    }
}
