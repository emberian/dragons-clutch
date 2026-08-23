// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    CodecError, FirstAdmittedTieV1, Reader, Writer, ADMISSION_NODE_ACCOUNT_TAG,
    ADMISSION_NODE_ACCOUNT_VERSION, CANDIDATE_FEED_ACCOUNT_TAG, CANDIDATE_FEED_ACCOUNT_VERSION,
    CANDIDATE_FEED_STAGE_ACCOUNT_TAG, CANDIDATE_FEED_STAGE_ACCOUNT_VERSION, CLEAR_WORK_ACCOUNT_TAG,
    CLEAR_WORK_ACCOUNT_VERSION, CLEAR_WORK_ACCOUNT_VERSION_V3, ECONOMIC_DOMAIN_ACCOUNT_TAG,
    ECONOMIC_DOMAIN_ACCOUNT_VERSION, EPOCH_BUDGET_ACCOUNT_TAG, EPOCH_BUDGET_ACCOUNT_VERSION,
    GENERAL_EPOCH_ACCOUNT_TAG, GENERAL_EPOCH_ACCOUNT_VERSION, ID_BYTES, MARKET_BINDING_ACCOUNT_TAG,
    MARKET_BINDING_ACCOUNT_VERSION, MARKET_RUNTIME_ACCOUNT_TAG, MARKET_RUNTIME_ACCOUNT_VERSION,
    MAX_ORDERS, MAX_ORDERS_U8, MAX_OUTCOMES, MAX_OUTCOMES_U8, MAX_QUANTIZED_ATOMS,
    MAX_QUANTIZED_ATOMS_U8, MAX_SLICES, MAX_SLICES_U16, SCORE_V2_Q_ACTIVE_RANK_BYTES,
    SCORE_V2_Q_RANK_CAPACITY, SELECTED_CANDIDATE_ACCOUNT_TAG, SELECTED_CANDIDATE_ACCOUNT_VERSION,
    WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION,
};

/// Domain prepended before hashing canonical [`EconomicDomainV2Transcript`]
/// bytes.
pub const ECONOMIC_DOMAIN_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/economic-domain/v2\0";
/// Domain prepended before hashing exact price semantics.
pub const PRICE_SEMANTICS_DIGEST_DOMAIN_V2: &[u8] = b"dragons-clutch/price-semantics/v2\0";
/// Domain prepended before hashing canonical General V2 Epoch semantics.
pub const EPOCH_SEMANTICS_DIGEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/epoch-semantics/v1\0";
/// Domain prepended before hashing a General V2 frozen order set.
pub const ORDER_SET_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/general-v2/order-set/v1\0";
/// Domain prepended before hashing a canonical quantized V3 witness body.
pub const QUANTIZED_WITNESS_BODY_DIGEST_DOMAIN_V3: &[u8] =
    b"dragons-clutch/price-measure-witness-body/v3\0";
/// Domain prepended before hashing exact settlement-slice witnesses.
pub const SETTLEMENT_WITNESS_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/settlement-witness/v1\0";
/// Domain prepended before hashing one complete General V2 candidate bundle.
pub const CANDIDATE_BUNDLE_DIGEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/candidate-bundle/v1\0";
/// Exact canonical EconomicDomainV2 transcript length, excluding the domain.
pub const ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES: usize = 4 + (5 * ID_BYTES) + 8 + 1 + 8 + 16 + 16;
/// Exact canonical General V2 Epoch-semantics transcript bytes.
pub const EPOCH_SEMANTICS_V1_TRANSCRIPT_BYTES: usize = ID_BYTES + (3 * 8);
/// Exact RelationV2 price-semantics body before the fixed-width price vector.
pub const RELATION_V2_PRICE_SEMANTICS_FIXED_BYTES: usize = 4 + (3 * ID_BYTES) + 8 + 1 + 8;
/// Exact canonical EconomicDomainV2 artifact-account bytes.
pub const ECONOMIC_DOMAIN_ACCOUNT_BYTES: usize =
    2 + ID_BYTES + ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES + 48 + 2;
/// Exact selected-candidate settlement-authority bytes.
pub const SELECTED_CANDIDATE_ACCOUNT_BYTES: usize = 789;
/// Exact SHA-256 resumable checkpoint bytes.
pub const SHA256_CHECKPOINT_BYTES: usize = 32 + 64 + 1 + 8;
/// FIPS 180-4 SHA-256 initial chaining words.
pub const SHA256_INITIAL_STATE_V1: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];
/// Exact Window account bytes.
pub const WINDOW_ACCOUNT_BYTES: usize = 565;
/// Exact AdmissionNode account bytes.
pub const ADMISSION_NODE_ACCOUNT_BYTES: usize = 743;
/// Exact CandidateFeed/Stage fixed header bytes.
pub const CANDIDATE_FEED_HEADER_BYTES: usize = 538;
/// Exact ClearWork fixed header, including its SHA checkpoint.
pub const CLEAR_WORK_HEADER_BYTES: usize = 672;
/// Exact resumable RelationV2 ClearWork V3 fixed header.
pub const CLEAR_WORK_V3_HEADER_BYTES: usize = 710;
/// Exact epoch Budget account bytes.
pub const EPOCH_BUDGET_ACCOUNT_BYTES: usize = 272;
/// Exact immutable Market-binding account bytes.
pub const MARKET_BINDING_ACCOUNT_BYTES: usize = 540;
/// Exact genesis-assisted MarketRuntime account bytes.
pub const MARKET_RUNTIME_ACCOUNT_BYTES: usize = 148;
/// Exact counted General V2 Epoch account bytes.
pub const GENERAL_EPOCH_ACCOUNT_BYTES: usize = 321;
/// Exact fixed header bytes in the quantized V3 witness-body transcript.
pub const QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES: usize = (4 * ID_BYTES) + (2 * 8) + 5;
/// Exact fixed header bytes in the General V2 candidate-bundle transcript.
pub const CANDIDATE_BUNDLE_V1_FIXED_BYTES: usize = (10 * ID_BYTES) + (5 * 8) + 2 + 7;
/// Existing settlement slice width: two two-byte legs, outcome, quantity.
pub const SETTLEMENT_SLICE_BYTES: usize = 13;
/// Exact quantized atom width: coordinate `u128`, mass `u64`.
pub const QUANTIZED_ATOM_BYTES: usize = 24;
/// Exact quantized price-measure witness schema admitted by General V2.
pub const PRICE_MEASURE_WITNESS_SCHEMA_V3: u8 = 3;
/// Exact integer-grid semantic version admitted by General V2.
pub const QUANTIZED_PRICE_MEASURE_SEMANTICS_V1: u8 = 1;

/// Validated nonzero 32-byte identity or digest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Id32([u8; ID_BYTES]);

impl Id32 {
    /// All-zero absent sentinel.
    pub const ZERO: Self = Self([0u8; ID_BYTES]);

    /// Construct a required nonzero identity.
    pub const fn new(bytes: [u8; ID_BYTES]) -> Result<Self, CodecError> {
        let mut index = 0usize;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Ok(Self(bytes));
            }
            index += 1;
        }
        Err(CodecError::ZeroIdentity)
    }

    /// Construct bytes without assigning live/absent meaning.
    pub const fn from_bytes(bytes: [u8; ID_BYTES]) -> Self {
        Self(bytes)
    }
    /// Return raw persisted bytes.
    pub const fn bytes(self) -> [u8; ID_BYTES] {
        self.0
    }
    /// Whether this is the absent sentinel.
    pub fn is_zero(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

/// Exact payer-owned rent compartment embedded in a deletable account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeletableRentOwnerV1 {
    /// Exact payer and sole refundable-principal recipient.
    pub payer: Id32,
    /// Full rent principal paid without hostile-prefund discount.
    pub refundable_principal: u64,
    /// Prefund observed before creation; routed to the neutral sink.
    pub donation_floor: u64,
}

impl DeletableRentOwnerV1 {
    /// Validate nonzero payer/principal and checked balance geometry.
    pub fn validate(self) -> Result<(), CodecError> {
        live(self.payer)?;
        if self.refundable_principal == 0
            || self
                .refundable_principal
                .checked_add(self.donation_floor)
                .is_none()
        {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }
}

/// Genesis-assisted mutable cursor for one immutable General V2 MarketBinding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketRuntimeV3AccountV1 {
    /// Immutable MarketBinding PDA anchoring this runtime.
    pub market_binding: Id32,
    /// Full Product successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id32,
    /// Exact next Epoch index admitted by `InitEpoch`.
    pub next_epoch_index: u64,
    /// Exact nonzero generation assigned to the next Epoch.
    pub next_epoch_generation: u64,
    /// Number of Epochs created through this runtime.
    pub created_epoch_count: u64,
    /// Number of those Epochs atomically retired.
    pub retired_epoch_count: u64,
    /// Disjoint runtime rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl MarketRuntimeV3AccountV1 {
    /// Validate the complete cursor and counted-retirement state.
    pub fn validate(self) -> Result<(), CodecError> {
        live(self.market_binding)?;
        live(self.market_instance_v2_id)?;
        self.rent.validate()?;
        if self.next_epoch_generation == 0
            || self.retired_epoch_count > self.created_epoch_count
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Return the authoritative live Epoch count.
    pub fn live_epoch_count(self) -> Result<u64, CodecError> {
        self.validate()?;
        self.created_epoch_count
            .checked_sub(self.retired_epoch_count)
            .ok_or(CodecError::ArithmeticOverflow)
    }

    /// Encode exactly [`MARKET_RUNTIME_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, MARKET_RUNTIME_ACCOUNT_BYTES)?;
        header(
            &mut w,
            MARKET_RUNTIME_ACCOUNT_TAG,
            MARKET_RUNTIME_ACCOUNT_VERSION,
        )?;
        w.bytes(&self.market_binding.bytes())?;
        w.bytes(&self.market_instance_v2_id.bytes())?;
        w.u64(self.next_epoch_index)?;
        w.u64(self.next_epoch_generation)?;
        w.u64(self.created_epoch_count)?;
        w.u64(self.retired_epoch_count)?;
        write_rent(&mut w, self.rent)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode and validate exactly [`MARKET_RUNTIME_ACCOUNT_BYTES`] hostile
    /// bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, MARKET_RUNTIME_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            MARKET_RUNTIME_ACCOUNT_TAG,
            MARKET_RUNTIME_ACCOUNT_VERSION,
        )?;
        let value = Self {
            market_binding: read_id(&mut r)?,
            market_instance_v2_id: read_id(&mut r)?,
            next_epoch_index: r.u64()?,
            next_epoch_generation: r.u64()?,
            created_epoch_count: r.u64()?,
            retired_epoch_count: r.u64()?,
            rent: read_rent(&mut r)?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Canonical lifecycle phase of one counted General V2 Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralEpochPhaseV1 {
    /// Orders may exist in later profiles; the frozen order-set identity is absent.
    Open = 0,
    /// Order set and all submission boundaries are frozen.
    Frozen = 1,
    /// Candidate selection is terminal, with zero or one live selected artifact.
    Finalized = 2,
}

impl GeneralEpochPhaseV1 {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Frozen => 1,
            Self::Finalized => 2,
        }
    }

    fn from_byte(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Frozen),
            2 => Ok(Self::Finalized),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// RelationV2-native counted General Epoch.
///
/// Each persisted fact has one owner: policy terms remain in MarketBinding,
/// schedule details remain in Window, and EconomicDomain owns its transcript.
/// This root stores only the exact joins, immutable Epoch inputs, phase, and
/// authoritative child counts required for atomic lifecycle transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochV6AccountV1 {
    /// Immutable MarketBinding PDA.
    pub market_binding: Id32,
    /// Mutable genesis-assisted MarketRuntime PDA.
    pub market_runtime: Id32,
    /// Full Product successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id32,
    /// Canonical EconomicDomainV2 artifact PDA.
    pub economic_domain: Id32,
    /// Canonical candidate Window PDA.
    pub window: Id32,
    /// Canonical root Budget PDA.
    pub budget: Id32,
    /// Frozen order-set identity; absent only while Open.
    pub order_set: Id32,
    /// Runtime-owned monotone Epoch index.
    pub epoch_index: u64,
    /// Runtime-owned nonzero retirement generation.
    pub generation: u64,
    /// Earliest slot at which FreezeEpoch may succeed.
    pub freeze_deadline_slot: u64,
    /// Actual freeze slot; absent as zero only while Open.
    pub frozen_slot: u64,
    /// Authoritative count of live AdmissionNodes.
    pub candidate_bundle_count: u32,
    /// Authoritative count of live ClearWork accounts.
    pub work_count: u32,
    /// Authoritative count of live SelectedCandidate artifacts.
    pub selected_candidate_count: u32,
    /// Disjoint Epoch rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Canonical lifecycle phase.
    pub phase: GeneralEpochPhaseV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl GeneralEpochV6AccountV1 {
    /// Validate joins, phase partition, generation, and exhaustive counts.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.market_binding,
            self.market_runtime,
            self.market_instance_v2_id,
            self.economic_domain,
            self.window,
            self.budget,
        ] {
            live(id)?;
        }
        self.rent.validate()?;
        if self.generation == 0
            || self.freeze_deadline_slot == 0
            || self.work_count > self.candidate_bundle_count
            || self.selected_candidate_count > 1
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        match self.phase {
            GeneralEpochPhaseV1::Open => {
                absent(self.order_set)?;
                if self.frozen_slot != 0
                    || self.candidate_bundle_count != 0
                    || self.work_count != 0
                    || self.selected_candidate_count != 0
                {
                    return Err(CodecError::InvalidState);
                }
            }
            GeneralEpochPhaseV1::Frozen => {
                live(self.order_set)?;
                if self.frozen_slot < self.freeze_deadline_slot
                    || self.selected_candidate_count != 0
                {
                    return Err(CodecError::InvalidState);
                }
            }
            GeneralEpochPhaseV1::Finalized => {
                live(self.order_set)?;
                if self.frozen_slot < self.freeze_deadline_slot {
                    return Err(CodecError::InvalidState);
                }
            }
        }
        Ok(())
    }

    /// Canonically derive this Epoch's semantic identity.
    pub fn semantics_digest<B: Sha256BackendV1>(&self, backend: &B) -> Result<Id32, CodecError> {
        epoch_semantics_digest_v1(
            backend,
            EpochSemanticsV1 {
                market_instance_v2_id: self.market_instance_v2_id,
                epoch_index: self.epoch_index,
                generation: self.generation,
                freeze_deadline_slot: self.freeze_deadline_slot,
            },
        )
    }

    /// Encode exactly [`GENERAL_EPOCH_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, GENERAL_EPOCH_ACCOUNT_BYTES)?;
        header(
            &mut w,
            GENERAL_EPOCH_ACCOUNT_TAG,
            GENERAL_EPOCH_ACCOUNT_VERSION,
        )?;
        for id in [
            self.market_binding,
            self.market_runtime,
            self.market_instance_v2_id,
            self.economic_domain,
            self.window,
            self.budget,
            self.order_set,
        ] {
            w.bytes(&id.bytes())?;
        }
        w.u64(self.epoch_index)?;
        w.u64(self.generation)?;
        w.u64(self.freeze_deadline_slot)?;
        w.u64(self.frozen_slot)?;
        w.u32(self.candidate_bundle_count)?;
        w.u32(self.work_count)?;
        w.u32(self.selected_candidate_count)?;
        write_rent(&mut w, self.rent)?;
        w.u8(self.phase.to_byte())?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode and validate exactly [`GENERAL_EPOCH_ACCOUNT_BYTES`] hostile
    /// bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, GENERAL_EPOCH_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            GENERAL_EPOCH_ACCOUNT_TAG,
            GENERAL_EPOCH_ACCOUNT_VERSION,
        )?;
        let value = Self {
            market_binding: read_id(&mut r)?,
            market_runtime: read_id(&mut r)?,
            market_instance_v2_id: read_id(&mut r)?,
            economic_domain: read_id(&mut r)?,
            window: read_id(&mut r)?,
            budget: read_id(&mut r)?,
            order_set: Id32::from_bytes(r.array()?),
            epoch_index: r.u64()?,
            generation: r.u64()?,
            freeze_deadline_slot: r.u64()?,
            frozen_slot: r.u64()?,
            candidate_bundle_count: r.u32()?,
            work_count: r.u32()?,
            selected_candidate_count: r.u32()?,
            rent: read_rent(&mut r)?,
            phase: GeneralEpochPhaseV1::from_byte(r.u8()?)?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Canonical typed inputs to one General V2 Epoch semantic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochSemanticsV1 {
    /// Full Product successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id32,
    /// Runtime-owned monotone index.
    pub epoch_index: u64,
    /// Runtime-owned nonzero retirement generation.
    pub generation: u64,
    /// Immutable earliest freeze slot.
    pub freeze_deadline_slot: u64,
}

impl EpochSemanticsV1 {
    /// Validate the semantic inputs.
    pub fn validate(self) -> Result<(), CodecError> {
        live(self.market_instance_v2_id)?;
        if self.generation == 0 || self.freeze_deadline_slot == 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact canonical hash transcript.
    pub fn encode(self) -> Result<[u8; EPOCH_SEMANTICS_V1_TRANSCRIPT_BYTES], CodecError> {
        self.validate()?;
        let mut out = [0u8; EPOCH_SEMANTICS_V1_TRANSCRIPT_BYTES];
        let mut w = Writer::exact(&mut out, EPOCH_SEMANTICS_V1_TRANSCRIPT_BYTES)?;
        w.bytes(&self.market_instance_v2_id.bytes())?;
        w.u64(self.epoch_index)?;
        w.u64(self.generation)?;
        w.u64(self.freeze_deadline_slot)?;
        w.finish()?;
        Ok(out)
    }
}

/// Canonical transcript hashed to authenticate exactly one RelationV2 domain.
///
/// The adapter projects `market_instance_v2_id` directly to RelationV2's
/// `market_semantics_digest`, not to an eight-byte nonce or legacy Market PDA.
/// `price_measure_policy_v1_id` must equal RelationV2's
/// `price_policy_digest`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicDomainV2Transcript {
    /// Exact RelationV2 semantic version.
    pub relation_version: u32,
    /// Full Product successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id32,
    /// Immutable epoch semantic identity.
    pub epoch_semantics_digest: Id32,
    /// Exact RelationV2 policy identity.
    pub relation_policy_id: Id32,
    /// Selected GenesisV2 price-measure policy identity.
    pub price_measure_policy_v1_id: Id32,
    /// Authenticated NativeClaimBasisV1 exact-body identity.
    pub native_claim_basis_id: Id32,
    /// Monotone epoch index.
    pub epoch_index: u64,
    /// Active outcome prefix.
    pub outcome_count: u8,
    /// Exact integer simplex scale.
    pub price_scale: u64,
    /// Inclusive canonical integer-coordinate domain minimum.
    pub coordinate_domain_min: u128,
    /// Inclusive canonical integer-coordinate domain maximum.
    pub coordinate_domain_max: u128,
}

impl EconomicDomainV2Transcript {
    /// Validate the complete domain shape.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.relation_version != 2
            || !(2..=MAX_OUTCOMES_U8).contains(&self.outcome_count)
            || self.price_scale == 0
            || self.coordinate_domain_min >= self.coordinate_domain_max
        {
            return Err(CodecError::InvalidState);
        }
        for id in [
            self.market_instance_v2_id,
            self.epoch_semantics_digest,
            self.relation_policy_id,
            self.price_measure_policy_v1_id,
            self.native_claim_basis_id,
        ] {
            live(id)?;
        }
        Ok(())
    }

    /// Encode the exact hash transcript. The caller hashes
    /// `ECONOMIC_DOMAIN_DIGEST_DOMAIN_V1 || encoded`.
    pub fn encode(self) -> Result<[u8; ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES], CodecError> {
        self.validate()?;
        let mut out = [0u8; ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES];
        let mut writer = Writer::exact(&mut out, ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES)?;
        writer.u32(self.relation_version)?;
        for id in [
            self.market_instance_v2_id,
            self.epoch_semantics_digest,
            self.relation_policy_id,
            self.price_measure_policy_v1_id,
            self.native_claim_basis_id,
        ] {
            writer.bytes(&id.bytes())?;
        }
        writer.u64(self.epoch_index)?;
        writer.u8(self.outcome_count)?;
        writer.u64(self.price_scale)?;
        writer.u128(self.coordinate_domain_min)?;
        writer.u128(self.coordinate_domain_max)?;
        writer.finish()?;
        Ok(out)
    }
}

/// SHA-256 backend seam for canonical General V2 digests.
///
/// An implementation must return FIPS 180-4 SHA-256 over the exact
/// concatenation of every supplied slice in order. Runtime implementations are
/// an explicit cryptographic trust boundary and require differential vectors
/// against a separately implemented one-shot backend before activation.
pub trait Sha256BackendV1 {
    /// Hash byte slices in order without inserting a length, separator, or
    /// other framing.
    fn sha256(&self, parts: &[&[u8]]) -> [u8; ID_BYTES];
}

/// Derive the canonical nonzero General V2 Epoch semantic identity.
pub fn epoch_semantics_digest_v1<B: Sha256BackendV1>(
    backend: &B,
    semantics: EpochSemanticsV1,
) -> Result<Id32, CodecError> {
    let encoded = semantics.encode()?;
    Id32::new(backend.sha256(&[EPOCH_SEMANTICS_DIGEST_DOMAIN_V1, &encoded]))
}

/// Derive the canonical nonzero identity of the frozen empty General V2 book.
///
/// The two-byte zero count is part of the transcript. Later nonempty order-set
/// work must extend this owner over exact canonical records rather than define
/// a second empty-book convention.
pub fn empty_order_set_digest_v1<B: Sha256BackendV1>(
    backend: &B,
    economic_domain_digest: Id32,
) -> Result<Id32, CodecError> {
    live(economic_domain_digest)?;
    let empty_count = 0u16.to_le_bytes();
    Id32::new(backend.sha256(&[
        ORDER_SET_DIGEST_DOMAIN_V1,
        &economic_domain_digest.bytes(),
        &empty_count,
    ]))
}

/// Exact typed opening of one funded-candidate commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCommitmentOpeningV1 {
    /// Parent Epoch PDA.
    pub epoch: Id32,
    /// General V2 MarketRuntime PDA, exactly equal to `MarketBindingV1.market`.
    pub market: Id32,
    /// Frozen RelationV2 policy identity.
    pub relation_policy_id: Id32,
    /// Frozen admission-policy identity.
    pub admission_policy_id: Id32,
    /// Frozen score-policy identity.
    pub score_policy_id: Id32,
    /// Window's actual frozen slot.
    pub frozen_slot: u64,
    /// Authority that must sign the reveal.
    pub submitter_authority: Id32,
    /// Immutable destination of the unique solver prize.
    pub solver_reward_destination: Id32,
    /// Exact candidate-bundle identity opened by the reveal.
    pub candidate_bundle_digest: Id32,
    /// Caller-chosen 32-byte commitment secret.
    pub secret: [u8; ID_BYTES],
}

impl CandidateCommitmentOpeningV1 {
    /// Validate every authenticated identity and the nonzero frozen slot.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.admission_policy_id,
            self.score_policy_id,
            self.submitter_authority,
            self.solver_reward_destination,
            self.candidate_bundle_digest,
        ] {
            live(id)?;
        }
        if self.frozen_slot == 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }
}

/// Recompute the exact ADR-0008 commitment without using it as a PDA seed.
pub fn candidate_commitment_v1<B: Sha256BackendV1>(
    backend: &B,
    opening: CandidateCommitmentOpeningV1,
) -> Result<Id32, CodecError> {
    opening.validate()?;
    let frozen_slot = opening.frozen_slot.to_le_bytes();
    Id32::new(backend.sha256(&[
        crate::CANDIDATE_COMMITMENT_DOMAIN_V1,
        &opening.epoch.bytes(),
        &opening.market.bytes(),
        &opening.relation_policy_id.bytes(),
        &opening.admission_policy_id.bytes(),
        &opening.score_policy_id.bytes(),
        &frozen_slot,
        &opening.submitter_authority.bytes(),
        &opening.solver_reward_destination.bytes(),
        &opening.candidate_bundle_digest.bytes(),
        &opening.secret,
    ]))
}

/// Derive the canonical EconomicDomainV2 identity from typed fields.
///
/// This function does not accept a caller-provided preimage: it validates and
/// encodes [`EconomicDomainV2Transcript`] itself, then hashes exactly
/// `ECONOMIC_DOMAIN_DIGEST_DOMAIN_V1 || transcript`.
pub fn economic_domain_digest_v2<B: Sha256BackendV1>(
    backend: &B,
    transcript: EconomicDomainV2Transcript,
) -> Result<Id32, CodecError> {
    let encoded = transcript.encode()?;
    Id32::new(backend.sha256(&[ECONOMIC_DOMAIN_DIGEST_DOMAIN_V1, &encoded]))
}

/// Canonical exact-price transcript whose digest must equal a price
/// certificate's `candidate_price_digest`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceSemanticsV2 {
    /// Canonical domain projected field-for-field into RelationV2.
    pub domain: EconomicDomainV2Transcript,
    /// Active exact integer prices followed by canonical zero padding.
    pub prices: [u64; MAX_OUTCOMES],
}

impl PriceSemanticsV2 {
    /// Validate exact simplex equality and inactive zero padding.
    pub fn validate(self) -> Result<(), CodecError> {
        self.domain.validate()?;
        let active = usize::from(self.domain.outcome_count);
        if self.prices[active..].iter().any(|price| *price != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let mut sum = 0u64;
        let mut index = 0usize;
        while index < active {
            sum = sum
                .checked_add(self.prices[index])
                .ok_or(CodecError::ArithmeticOverflow)?;
            index += 1;
        }
        if sum != self.domain.price_scale {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
}

/// Hash the exact canonical price-semantics transcript.
///
/// This is a byte-exact reproduction of RelationV2's semantic owner, not a
/// second identity. The preimage is `PRICE_SEMANTICS_DIGEST_DOMAIN_V2 ||
/// relation_version_le || market_semantics_digest || epoch_semantics_digest ||
/// price_policy_digest || epoch_index_le || outcome_count || price_scale_le ||
/// all_16_prices_le`. Product `MarketInstanceV2Id` projects to market semantics
/// and `PriceMeasurePolicyV1Id` projects to price policy. Inactive price slots
/// are canonical zeroes and remain inside the hashed fixed-width vector.
pub fn price_semantics_digest_v2<B: Sha256BackendV1>(
    backend: &B,
    semantics: PriceSemanticsV2,
) -> Result<Id32, CodecError> {
    semantics.validate()?;
    let mut fixed = [0u8; RELATION_V2_PRICE_SEMANTICS_FIXED_BYTES];
    let mut writer = Writer::exact(&mut fixed, RELATION_V2_PRICE_SEMANTICS_FIXED_BYTES)?;
    writer.u32(semantics.domain.relation_version)?;
    for id in [
        semantics.domain.market_instance_v2_id,
        semantics.domain.epoch_semantics_digest,
        semantics.domain.price_measure_policy_v1_id,
    ] {
        writer.bytes(&id.bytes())?;
    }
    writer.u64(semantics.domain.epoch_index)?;
    writer.u8(semantics.domain.outcome_count)?;
    writer.u64(semantics.domain.price_scale)?;
    writer.finish()?;
    let mut prices = [0u8; MAX_OUTCOMES * 8];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        let at = index * 8;
        prices[at..at + 8].copy_from_slice(&semantics.prices[index].to_le_bytes());
        index += 1;
    }
    Id32::new(backend.sha256(&[PRICE_SEMANTICS_DIGEST_DOMAIN_V2, &fixed, &prices]))
}

/// Immutable per-Epoch owner of the canonical EconomicDomainV2 transcript.
///
/// The digest is derived, never persisted as a parallel truth. `InitEpoch`
/// creates this artifact atomically with its successor Epoch; `CloseEpoch`
/// must close it and coalesce its exact rent credit before the Epoch tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicDomainV2AccountV1 {
    /// Parent successor Epoch account.
    pub epoch: Id32,
    /// Canonical fields whose SHA-256 identity binds RelationV2 and the price
    /// certificate adapter.
    pub transcript: EconomicDomainV2Transcript,
    /// Disjoint artifact rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl EconomicDomainV2AccountV1 {
    /// Validate the typed transcript, parent, rent owner, and reserved bytes.
    pub fn validate(self) -> Result<(), CodecError> {
        live(self.epoch)?;
        self.transcript.validate()?;
        self.rent.validate()?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode exactly [`ECONOMIC_DOMAIN_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, ECONOMIC_DOMAIN_ACCOUNT_BYTES)?;
        header(
            &mut w,
            ECONOMIC_DOMAIN_ACCOUNT_TAG,
            ECONOMIC_DOMAIN_ACCOUNT_VERSION,
        )?;
        w.bytes(&self.epoch.bytes())?;
        write_economic_domain_transcript(&mut w, self.transcript)?;
        write_rent(&mut w, self.rent)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode and validate exactly [`ECONOMIC_DOMAIN_ACCOUNT_BYTES`] hostile
    /// bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, ECONOMIC_DOMAIN_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            ECONOMIC_DOMAIN_ACCOUNT_TAG,
            ECONOMIC_DOMAIN_ACCOUNT_VERSION,
        )?;
        let value = Self {
            epoch: read_id(&mut r)?,
            transcript: read_economic_domain_transcript(&mut r)?,
            rent: read_rent(&mut r)?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Candidate route whose checked identity becomes the final settlement ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementCandidateKindV1 {
    /// Final ID is the verified RelationV2 economic candidate digest.
    Direct = 0,
    /// Final ID is the checked covered-dealer economic candidate digest.
    CoveredDealer = 1,
}

impl SettlementCandidateKindV1 {
    /// Return the exact one-byte wire value.
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::Direct => 0,
            Self::CoveredDealer => 1,
        }
    }

    /// Decode one exact one-byte wire value.
    pub fn from_byte(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Direct),
            1 => Ok(Self::CoveredDealer),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Immutable Market/Genesis/Product/policy join required by every General V2
/// epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBindingV1 {
    /// General V2 MarketRuntime PDA; never the Product MarketInstanceV2 ID.
    pub market: Id32,
    /// Full `MarketGenesisProfileV2Id`; never a V1 profile reinterpretation.
    pub market_genesis_profile_v2_id: Id32,
    /// Full `MarketInstanceV2Id`; never lowered to the legacy nonce.
    pub market_instance_v2_id: Id32,
    /// Separate recurring `SeriesPlanV5Id` provenance.
    pub series_plan_v5_id: Id32,
    /// Exact `SeriesFundingTermsV2Id`; never inferred from mutable balances.
    pub series_funding_terms_v2_id: Id32,
    /// RelationV2 policy identity.
    pub relation_policy_id: Id32,
    /// Typed `PriceMeasurePolicyV1Id` selected by Genesis V2. This policy
    /// admits quantized V3 degrees zero through three.
    pub price_measure_policy_v1_id: Id32,
    /// Exact NativeClaimBasisV1 body identity.
    pub native_claim_basis_id: Id32,
    /// Funded admission policy identity.
    pub admission_policy_id: Id32,
    /// Distinct score policy identity for the 88-byte active key.
    pub score_policy_id: Id32,
    /// Existing settlement/allocation policy identity.
    pub settlement_policy_id: Id32,
    /// Immutable donation/penalty sink.
    pub neutral_sink: Id32,
    /// Exact integer simplex scale.
    pub price_scale: u64,
    /// Commit subinterval span.
    pub commit_span_slots: u64,
    /// Reveal subinterval span.
    pub reveal_span_slots: u64,
    /// Verification interval span.
    pub verification_span_slots: u64,
    /// Per-node admission bond.
    pub bond_lamports: u64,
    /// Checked-invalidity penalty.
    pub invalidity_penalty: u64,
    /// Unrevealed-commitment abandonment penalty.
    pub abandonment_penalty: u64,
    /// Prepaid permissionless node cleanup reward.
    pub node_cleanup_reward: u64,
    /// Reward for price-certificate checking.
    pub price_check_reward: u64,
    /// Reward per newly checked order.
    pub order_reward: u64,
    /// Reward per newly checked settlement slice.
    pub slice_reward: u64,
    /// Reward for a completed verdict.
    pub completion_reward: u64,
    /// Reward for closing ClearWork.
    pub work_close_reward: u64,
    /// Reward for closing a feed/stage.
    pub feed_close_reward: u64,
    /// Root freeze reward.
    pub freeze_reward: u64,
    /// Root finalization reward.
    pub finalize_reward: u64,
    /// Unique selected-solver prize.
    pub solver_prize: u64,
    /// Root retirement reward.
    pub root_close_reward: u64,
    /// Exact RelationV2 version, currently two.
    pub relation_version: u32,
    /// Active market outcome width.
    pub outcome_count: u8,
    /// Quantized V3 basis degree, zero through three.
    pub basis_degree: u8,
    /// Must equal 88.
    pub rank_key_len: u8,
    /// Bit zero admits Direct; bit one reserves CoveredDealer but does not
    /// activate it without the Dealer family.
    pub candidate_kind_mask: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl MarketBindingV1 {
    /// Validate immutable policy geometry and identities.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.market,
            self.market_genesis_profile_v2_id,
            self.market_instance_v2_id,
            self.series_plan_v5_id,
            self.series_funding_terms_v2_id,
            self.relation_policy_id,
            self.price_measure_policy_v1_id,
            self.native_claim_basis_id,
            self.admission_policy_id,
            self.score_policy_id,
            self.settlement_policy_id,
            self.neutral_sink,
        ] {
            live(id)?;
        }
        if self.relation_version != 2
            || !(2..=MAX_OUTCOMES_U8).contains(&self.outcome_count)
            || self.basis_degree > 3
            || self.outcome_count <= self.basis_degree
            || usize::from(self.rank_key_len) != SCORE_V2_Q_ACTIVE_RANK_BYTES
            || self.price_scale == 0
            || self.commit_span_slots == 0
            || self.reveal_span_slots == 0
            || self.verification_span_slots == 0
            || self.bond_lamports == 0
            || self.node_cleanup_reward == 0
            || self.price_check_reward == 0
            || self.order_reward == 0
            || self.slice_reward == 0
            || self.completion_reward == 0
            || self.work_close_reward == 0
            || self.feed_close_reward == 0
            || self.freeze_reward == 0
            || self.finalize_reward == 0
            || self.solver_prize == 0
            || self.root_close_reward == 0
            || self.invalidity_penalty == 0
            || self.invalidity_penalty > self.bond_lamports
            || self.abandonment_penalty == 0
            || self.abandonment_penalty > self.bond_lamports
            || self.candidate_kind_mask == 0
            || self.candidate_kind_mask & !0b11 != 0
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        self.commit_span_slots
            .checked_add(self.reveal_span_slots)
            .and_then(|v| v.checked_add(self.verification_span_slots))
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Encode exactly [`MARKET_BINDING_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, MARKET_BINDING_ACCOUNT_BYTES)?;
        header(
            &mut w,
            MARKET_BINDING_ACCOUNT_TAG,
            MARKET_BINDING_ACCOUNT_VERSION,
        )?;
        for id in [
            self.market,
            self.market_genesis_profile_v2_id,
            self.market_instance_v2_id,
            self.series_plan_v5_id,
            self.series_funding_terms_v2_id,
            self.relation_policy_id,
            self.price_measure_policy_v1_id,
            self.native_claim_basis_id,
            self.admission_policy_id,
            self.score_policy_id,
            self.settlement_policy_id,
            self.neutral_sink,
        ] {
            w.bytes(&id.bytes())?;
        }
        for value in [
            self.price_scale,
            self.commit_span_slots,
            self.reveal_span_slots,
            self.verification_span_slots,
            self.bond_lamports,
            self.invalidity_penalty,
            self.abandonment_penalty,
            self.node_cleanup_reward,
            self.price_check_reward,
            self.order_reward,
            self.slice_reward,
            self.completion_reward,
            self.work_close_reward,
            self.feed_close_reward,
            self.freeze_reward,
            self.finalize_reward,
            self.solver_prize,
            self.root_close_reward,
        ] {
            w.u64(value)?;
        }
        w.u32(self.relation_version)?;
        for value in [
            self.outcome_count,
            self.basis_degree,
            self.rank_key_len,
            self.candidate_kind_mask,
            self.stored_bump,
            self.flags,
        ] {
            w.u8(value)?;
        }
        w.finish()
    }

    /// Decode and validate exactly [`MARKET_BINDING_ACCOUNT_BYTES`] hostile
    /// bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, MARKET_BINDING_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            MARKET_BINDING_ACCOUNT_TAG,
            MARKET_BINDING_ACCOUNT_VERSION,
        )?;
        let value = Self {
            market: read_id(&mut r)?,
            market_genesis_profile_v2_id: read_id(&mut r)?,
            market_instance_v2_id: read_id(&mut r)?,
            series_plan_v5_id: read_id(&mut r)?,
            series_funding_terms_v2_id: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            price_measure_policy_v1_id: read_id(&mut r)?,
            native_claim_basis_id: read_id(&mut r)?,
            admission_policy_id: read_id(&mut r)?,
            score_policy_id: read_id(&mut r)?,
            settlement_policy_id: read_id(&mut r)?,
            neutral_sink: read_id(&mut r)?,
            price_scale: r.u64()?,
            commit_span_slots: r.u64()?,
            reveal_span_slots: r.u64()?,
            verification_span_slots: r.u64()?,
            bond_lamports: r.u64()?,
            invalidity_penalty: r.u64()?,
            abandonment_penalty: r.u64()?,
            node_cleanup_reward: r.u64()?,
            price_check_reward: r.u64()?,
            order_reward: r.u64()?,
            slice_reward: r.u64()?,
            completion_reward: r.u64()?,
            work_close_reward: r.u64()?,
            feed_close_reward: r.u64()?,
            freeze_reward: r.u64()?,
            finalize_reward: r.u64()?,
            solver_prize: r.u64()?,
            root_close_reward: r.u64()?,
            relation_version: r.u32()?,
            outcome_count: r.u8()?,
            basis_degree: r.u8()?,
            rank_key_len: r.u8()?,
            candidate_kind_mask: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Exact candidate-local capitalization decomposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFundingV1 {
    /// Price/order/slice/completion/work-close rewards moved into ClearWork.
    pub work_reward_reserve: u64,
    /// Feed-close reward retained with the feed.
    pub feed_close_reward: u64,
    /// Node-close reward retained with the AdmissionNode.
    pub node_cleanup_reward: u64,
    /// Exact node allocation paid at commitment.
    pub node_allocation: u64,
    /// Exact feed allocation paid only after reveal fixes its active width.
    pub feed_allocation: u64,
    /// Exact ClearWork allocation paid only after reveal fixes its dimensions.
    pub work_allocation: u64,
    /// Exact payer debit at commitment.
    pub commit_payer_funding: u64,
    /// Exact additional payer debit at reveal.
    pub reveal_payer_funding: u64,
    /// Auditable lifetime payer funding, never a single-transition debit.
    pub lifetime_payer_funding: u64,
    /// Node post-funding balance including its hostile-prefund floor.
    pub node_balance_with_prefund: u64,
    /// Feed post-funding balance including its hostile-prefund floor.
    pub feed_balance_with_prefund: u64,
    /// Work post-funding balance including its hostile-prefund floor.
    pub work_balance_with_prefund: u64,
    /// Auditable combined post-funding balances including every prefund floor.
    pub lifetime_balance_with_prefunds: u64,
}

/// Derive exact candidate funding from immutable policy, active widths, and
/// independently owned rent compartments.
pub fn required_candidate_funding_v1(
    policy: MarketBindingV1,
    order_count: u8,
    slice_count: u16,
    node_rent: DeletableRentOwnerV1,
    feed_rent: DeletableRentOwnerV1,
    work_rent: DeletableRentOwnerV1,
) -> Result<CandidateFundingV1, CodecError> {
    policy.validate()?;
    node_rent.validate()?;
    feed_rent.validate()?;
    work_rent.validate()?;
    if order_count > MAX_ORDERS_U8 || slice_count > MAX_SLICES_U16 {
        return Err(CodecError::InvalidCount);
    }
    let order_rewards = policy
        .order_reward
        .checked_mul(u64::from(order_count))
        .ok_or(CodecError::ArithmeticOverflow)?;
    let slice_rewards = policy
        .slice_reward
        .checked_mul(u64::from(slice_count))
        .ok_or(CodecError::ArithmeticOverflow)?;
    let work_reward_reserve = policy
        .price_check_reward
        .checked_add(order_rewards)
        .and_then(|value| value.checked_add(slice_rewards))
        .and_then(|value| value.checked_add(policy.completion_reward))
        .and_then(|value| value.checked_add(policy.work_close_reward))
        .ok_or(CodecError::ArithmeticOverflow)?;
    let node_allocation = node_rent
        .refundable_principal
        .checked_add(policy.bond_lamports)
        .and_then(|value| value.checked_add(policy.node_cleanup_reward))
        .ok_or(CodecError::ArithmeticOverflow)?;
    let feed_allocation = feed_rent
        .refundable_principal
        .checked_add(policy.feed_close_reward)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let work_allocation = work_rent
        .refundable_principal
        .checked_add(work_reward_reserve)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let commit_payer_funding = node_allocation;
    let reveal_payer_funding = feed_allocation
        .checked_add(work_allocation)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let lifetime_payer_funding = commit_payer_funding
        .checked_add(reveal_payer_funding)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let node_balance_with_prefund = node_allocation
        .checked_add(node_rent.donation_floor)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let feed_balance_with_prefund = feed_allocation
        .checked_add(feed_rent.donation_floor)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let work_balance_with_prefund = work_allocation
        .checked_add(work_rent.donation_floor)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let lifetime_balance_with_prefunds = node_balance_with_prefund
        .checked_add(feed_balance_with_prefund)
        .and_then(|value| value.checked_add(work_balance_with_prefund))
        .ok_or(CodecError::ArithmeticOverflow)?;
    Ok(CandidateFundingV1 {
        work_reward_reserve,
        feed_close_reward: policy.feed_close_reward,
        node_cleanup_reward: policy.node_cleanup_reward,
        node_allocation,
        feed_allocation,
        work_allocation,
        commit_payer_funding,
        reveal_payer_funding,
        lifetime_payer_funding,
        node_balance_with_prefund,
        feed_balance_with_prefund,
        work_balance_with_prefund,
        lifetime_balance_with_prefunds,
    })
}

/// Successor Window with two submission subintervals, one verification
/// interval, exhaustive node counts, and one canonical best rank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateWindowV4AccountV1 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Relation policy.
    pub relation_policy_id: Id32,
    /// Admission policy.
    pub admission_policy_id: Id32,
    /// Score policy.
    pub score_policy_id: Id32,
    /// Earliest freeze slot.
    pub freeze_deadline_slot: u64,
    /// Actual freeze slot.
    pub frozen_slot: u64,
    /// Commit/reveal boundary.
    pub reveal_opens_slot: u64,
    /// Reveal/verification boundary.
    pub submission_closes_slot: u64,
    /// Hard verification boundary.
    pub verification_closes_slot: u64,
    /// One-way selection slot.
    pub finalized_slot: u64,
    /// Newest live admission node.
    pub admission_head: Id32,
    /// Best valid node under the rank.
    pub best_candidate_node: Id32,
    /// Typed final ID of `best_candidate_node`.
    pub best_settlement_candidate_id: Id32,
    /// Immutable selected-candidate artifact materialized by finalization.
    pub selected_candidate_artifact: Id32,
    /// Canonical 88-byte active rank plus eight zero bytes.
    pub best_rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
    /// Total admitted nodes.
    pub admitted_count: u64,
    /// Successfully revealed nodes.
    pub revealed_count: u64,
    /// Checked verdicts.
    pub verdict_count: u64,
    /// Checked valid verdicts.
    pub valid_verdict_count: u64,
    /// Unrevealed expiries.
    pub expired_commitment_count: u64,
    /// Revealed but unverified expiries.
    pub expired_unverified_count: u64,
    /// Live reverse-linked nodes.
    pub live_node_count: u64,
    /// Deleted reverse-linked nodes.
    pub closed_node_count: u64,
    /// First-admitted ordinal of `best_candidate_node`.
    pub best_ordinal: u64,
    /// Parent counted Epoch generation.
    pub epoch_generation: u64,
    /// Disjoint Window rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Must equal 88.
    pub rank_key_len: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

/// Semantic-owner proof that a General V2 Window has finalized its one-way
/// selection and that every reverse-linked AdmissionNode has been deleted.
///
/// This proof deliberately retains the historical selected-artifact identity:
/// a separate authenticated selected-family close proves that account absent
/// and decrements its Epoch count exactly once. A Window alone cannot
/// authorize root retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateWindowRetirementDispositionV1 {
    market: Id32,
    epoch_account: Id32,
    epoch_generation: u64,
    rent: DeletableRentOwnerV1,
    admitted_count: u64,
    closed_node_count: u64,
    selected_candidate_artifact: Id32,
}

impl CandidateWindowRetirementDispositionV1 {
    /// Parent Market authenticated by the Window codec.
    pub const fn market(self) -> Id32 {
        self.market
    }

    /// Parent Epoch account identity authenticated by the Window codec.
    pub const fn epoch_account(self) -> Id32 {
        self.epoch_account
    }

    /// Exact parent generation authenticated by the Window codec.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }

    /// Independently owned Window rent principal and donation floor.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Total number of nodes ever admitted by this Window.
    pub const fn admitted_count(self) -> u64 {
        self.admitted_count
    }

    /// Total number of nodes deleted through the reverse-linked close path.
    pub const fn closed_node_count(self) -> u64 {
        self.closed_node_count
    }

    /// Historical selected-artifact identity, or the all-zero sentinel when
    /// this Window finalized without a valid submitted candidate.
    pub const fn selected_candidate_artifact(self) -> Id32 {
        self.selected_candidate_artifact
    }
}

impl CandidateWindowV4AccountV1 {
    /// Validate canonical count, schedule, rank, and rent state.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.admission_policy_id,
            self.score_policy_id,
        ] {
            live(id)?;
        }
        self.rent.validate()?;
        if self.freeze_deadline_slot == 0
            || self.epoch_generation == 0
            || usize::from(self.rank_key_len) != SCORE_V2_Q_ACTIVE_RANK_BYTES
            || self.flags != 0
            || self.admitted_count > u64::from(u32::MAX)
            || self.revealed_count > self.admitted_count
            || self.verdict_count > self.revealed_count
            || self.valid_verdict_count > self.verdict_count
            || self.expired_commitment_count > self.admitted_count
            || self.expired_unverified_count > self.revealed_count
            || self
                .revealed_count
                .checked_add(self.expired_commitment_count)
                .ok_or(CodecError::ArithmeticOverflow)?
                > self.admitted_count
            || self
                .verdict_count
                .checked_add(self.expired_unverified_count)
                .ok_or(CodecError::ArithmeticOverflow)?
                > self.revealed_count
            || self
                .live_node_count
                .checked_add(self.closed_node_count)
                .ok_or(CodecError::ArithmeticOverflow)?
                != self.admitted_count
        {
            return Err(CodecError::InvalidState);
        }
        if self.best_rank_key[SCORE_V2_Q_ACTIVE_RANK_BYTES..]
            .iter()
            .any(|b| *b != 0)
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        if self.live_node_count == 0 {
            absent(self.admission_head)?;
        } else {
            live(self.admission_head)?;
        }
        if self.frozen_slot == 0 {
            if self.reveal_opens_slot != 0
                || self.submission_closes_slot != 0
                || self.verification_closes_slot != 0
                || self.finalized_slot != 0
                || self.admitted_count != 0
            {
                return Err(CodecError::InvalidState);
            }
        } else if !(self.frozen_slot >= self.freeze_deadline_slot
            && self.frozen_slot < self.reveal_opens_slot
            && self.reveal_opens_slot < self.submission_closes_slot
            && self.submission_closes_slot < self.verification_closes_slot)
        {
            return Err(CodecError::InvalidState);
        }
        if self.finalized_slot == 0 {
            absent(self.selected_candidate_artifact)?;
            if self.valid_verdict_count == 0 {
                absent(self.best_candidate_node)?;
                absent(self.best_settlement_candidate_id)?;
                if self.best_ordinal != 0 || self.best_rank_key != [0u8; SCORE_V2_Q_RANK_CAPACITY] {
                    return Err(CodecError::NonCanonicalPadding);
                }
            } else {
                live(self.best_candidate_node)?;
                live(self.best_settlement_candidate_id)?;
                if self.best_ordinal == 0 || self.best_ordinal > self.admitted_count {
                    return Err(CodecError::InvalidCount);
                }
                validate_rank_candidate_and_ordinal(
                    self.best_rank_key,
                    self.best_settlement_candidate_id,
                    self.best_ordinal,
                )?;
            }
        } else {
            if self.finalized_slot < self.submission_closes_slot
                || self
                    .revealed_count
                    .checked_add(self.expired_commitment_count)
                    .ok_or(CodecError::ArithmeticOverflow)?
                    != self.admitted_count
                || self
                    .verdict_count
                    .checked_add(self.expired_unverified_count)
                    .ok_or(CodecError::ArithmeticOverflow)?
                    != self.revealed_count
            {
                return Err(CodecError::InvalidState);
            }
            absent(self.best_candidate_node)?;
            absent(self.best_settlement_candidate_id)?;
            if self.best_ordinal != 0 || self.best_rank_key != [0u8; SCORE_V2_Q_RANK_CAPACITY] {
                return Err(CodecError::NonCanonicalPadding);
            }
            if self.valid_verdict_count == 0 {
                absent(self.selected_candidate_artifact)?;
            } else {
                live(self.selected_candidate_artifact)?;
            }
        }
        Ok(())
    }

    /// Consume the semantic Window terminality check for atomic root close.
    ///
    /// Finalization is one-way, the reverse-linked head must be absent, and
    /// every admitted node must already have traversed its authoritative close
    /// transition. The selected artifact is only a historical pointer here;
    /// its independently counted account family still requires exact absence
    /// evidence before the Epoch root may retire.
    pub fn retirement_disposition(
        self,
    ) -> Result<CandidateWindowRetirementDispositionV1, CodecError> {
        self.validate()?;
        if self.finalized_slot == 0
            || self.live_node_count != 0
            || !self.admission_head.is_zero()
            || self.closed_node_count != self.admitted_count
        {
            return Err(CodecError::InvalidState);
        }
        Ok(CandidateWindowRetirementDispositionV1 {
            market: self.market,
            epoch_account: self.epoch,
            epoch_generation: self.epoch_generation,
            rent: self.rent,
            admitted_count: self.admitted_count,
            closed_node_count: self.closed_node_count,
            selected_candidate_artifact: self.selected_candidate_artifact,
        })
    }

    /// Encode exactly [`WINDOW_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, WINDOW_ACCOUNT_BYTES)?;
        header(&mut w, WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION)?;
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.admission_policy_id,
            self.score_policy_id,
        ] {
            w.bytes(&id.bytes())?;
        }
        for value in [
            self.freeze_deadline_slot,
            self.frozen_slot,
            self.reveal_opens_slot,
            self.submission_closes_slot,
            self.verification_closes_slot,
            self.finalized_slot,
        ] {
            w.u64(value)?;
        }
        for id in [
            self.admission_head,
            self.best_candidate_node,
            self.best_settlement_candidate_id,
            self.selected_candidate_artifact,
        ] {
            w.bytes(&id.bytes())?;
        }
        w.bytes(&self.best_rank_key)?;
        for value in [
            self.admitted_count,
            self.revealed_count,
            self.verdict_count,
            self.valid_verdict_count,
            self.expired_commitment_count,
            self.expired_unverified_count,
            self.live_node_count,
            self.closed_node_count,
            self.best_ordinal,
            self.epoch_generation,
        ] {
            w.u64(value)?;
        }
        write_rent(&mut w, self.rent)?;
        w.u8(self.rank_key_len)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode and validate exactly [`WINDOW_ACCOUNT_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, WINDOW_ACCOUNT_BYTES)?;
        check_header(&mut r, WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION)?;
        let value = Self {
            epoch: read_id(&mut r)?,
            market: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            admission_policy_id: read_id(&mut r)?,
            score_policy_id: read_id(&mut r)?,
            freeze_deadline_slot: r.u64()?,
            frozen_slot: r.u64()?,
            reveal_opens_slot: r.u64()?,
            submission_closes_slot: r.u64()?,
            verification_closes_slot: r.u64()?,
            finalized_slot: r.u64()?,
            admission_head: Id32::from_bytes(r.array()?),
            best_candidate_node: Id32::from_bytes(r.array()?),
            best_settlement_candidate_id: Id32::from_bytes(r.array()?),
            selected_candidate_artifact: Id32::from_bytes(r.array()?),
            best_rank_key: r.array()?,
            admitted_count: r.u64()?,
            revealed_count: r.u64()?,
            verdict_count: r.u64()?,
            valid_verdict_count: r.u64()?,
            expired_commitment_count: r.u64()?,
            expired_unverified_count: r.u64()?,
            live_node_count: r.u64()?,
            closed_node_count: r.u64()?,
            best_ordinal: r.u64()?,
            epoch_generation: r.u64()?,
            rent: read_rent(&mut r)?,
            rank_key_len: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Settlement authority materialized atomically by selection.
///
/// Before finalization the Window is the only owner of working-best fields.
/// Finalization copies every downstream fact here, zeroes the Window's
/// working-best fields, stores only this artifact's identity in the Window,
/// and increments the Epoch selected-artifact count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedCandidateV1AccountV1 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Source Window.
    pub window: Id32,
    /// Immutable Market-binding account.
    pub market_binding: Id32,
    /// Historical source AdmissionNode; it may retire after materialization.
    pub source_admission_node: Id32,
    /// Selected sealed feed retained as counted settlement data authority.
    pub selected_feed: Id32,
    /// Frozen order-set owner.
    pub order_set: Id32,
    /// Derived EconomicDomainV2 digest.
    pub economic_domain_digest: Id32,
    /// Commitment-opened bundle digest.
    pub candidate_bundle_digest: Id32,
    /// Typed final settlement candidate identity.
    pub settlement_candidate_id: Id32,
    /// Verified base RelationV2 candidate identity.
    pub base_relation_candidate_id: Id32,
    /// Exact settlement allocation-witness digest.
    pub settlement_witness_digest: Id32,
    /// RelationV2 policy identity.
    pub relation_policy_id: Id32,
    /// Selected `PriceMeasurePolicyV1Id` bytes.
    pub price_measure_policy_v1_id: Id32,
    /// Authenticated NativeClaimBasisV1 exact-body identity.
    pub native_claim_basis_id: Id32,
    /// Canonical price-semantics digest checked by the certificate.
    pub candidate_price_digest: Id32,
    /// Authenticated V3 certificate body digest.
    pub price_body_digest: Id32,
    /// Score policy identity.
    pub score_policy_id: Id32,
    /// Unique solver-prize destination copied from admission.
    pub solver_reward_destination: Id32,
    /// Full canonical selected rank.
    pub rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Window-owned one-based admission ordinal.
    pub ordinal: u64,
    /// Selection slot.
    pub selected_slot: u64,
    /// Exact retained-feed slice count.
    pub slice_count: u16,
    /// Next slice to materialize into counted settlement state.
    pub next_slice_index: u16,
    /// Disjoint artifact rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Direct or CoveredDealer final-ID provenance.
    pub candidate_kind: SettlementCandidateKindV1,
    /// Must equal V3.
    pub price_witness_schema: u8,
    /// Must equal quantized integer-grid semantics V1.
    pub quantized_semantics_version: u8,
    /// Must equal 88.
    pub rank_key_len: u8,
    /// Entitlement phase: open, frozen, or fully materialized.
    pub entitlement_state: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl SelectedCandidateV1AccountV1 {
    /// Validate complete downstream identity, score, certificate, and rent
    /// ownership.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.market,
            self.window,
            self.market_binding,
            self.source_admission_node,
            self.selected_feed,
            self.order_set,
            self.economic_domain_digest,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.settlement_witness_digest,
            self.relation_policy_id,
            self.price_measure_policy_v1_id,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_body_digest,
            self.score_policy_id,
            self.solver_reward_destination,
        ] {
            live(id)?;
        }
        self.rent.validate()?;
        if self.epoch_generation == 0
            || self.ordinal == 0
            || self.selected_slot == 0
            || self.price_witness_schema != PRICE_MEASURE_WITNESS_SCHEMA_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_V1
            || usize::from(self.rank_key_len) != SCORE_V2_Q_ACTIVE_RANK_BYTES
            || self.slice_count > MAX_SLICES_U16
            || self.next_slice_index > self.slice_count
            || self.entitlement_state > 2
            || (self.slice_count == 0 && self.entitlement_state != 2)
            || (self.entitlement_state == 0 && self.next_slice_index != 0)
            || (self.entitlement_state == 1 && self.next_slice_index == self.slice_count)
            || (self.entitlement_state == 2 && self.next_slice_index != self.slice_count)
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        if self.candidate_kind == SettlementCandidateKindV1::Direct
            && self.settlement_candidate_id != self.base_relation_candidate_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        validate_rank_candidate_and_ordinal(
            self.rank_key,
            self.settlement_candidate_id,
            self.ordinal,
        )
    }

    /// Encode exactly [`SELECTED_CANDIDATE_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, SELECTED_CANDIDATE_ACCOUNT_BYTES)?;
        header(
            &mut w,
            SELECTED_CANDIDATE_ACCOUNT_TAG,
            SELECTED_CANDIDATE_ACCOUNT_VERSION,
        )?;
        for id in [
            self.epoch,
            self.market,
            self.window,
            self.market_binding,
            self.source_admission_node,
            self.selected_feed,
            self.order_set,
            self.economic_domain_digest,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.settlement_witness_digest,
            self.relation_policy_id,
            self.price_measure_policy_v1_id,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_body_digest,
            self.score_policy_id,
            self.solver_reward_destination,
        ] {
            w.bytes(&id.bytes())?;
        }
        w.bytes(&self.rank_key)?;
        w.u64(self.epoch_generation)?;
        w.u64(self.ordinal)?;
        w.u64(self.selected_slot)?;
        w.u16(self.slice_count)?;
        w.u16(self.next_slice_index)?;
        write_rent(&mut w, self.rent)?;
        w.u8(self.candidate_kind.to_byte())?;
        w.u8(self.price_witness_schema)?;
        w.u8(self.quantized_semantics_version)?;
        w.u8(self.rank_key_len)?;
        w.u8(self.entitlement_state)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode and validate exactly [`SELECTED_CANDIDATE_ACCOUNT_BYTES`]
    /// hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, SELECTED_CANDIDATE_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            SELECTED_CANDIDATE_ACCOUNT_TAG,
            SELECTED_CANDIDATE_ACCOUNT_VERSION,
        )?;
        let value = Self {
            epoch: read_id(&mut r)?,
            market: read_id(&mut r)?,
            window: read_id(&mut r)?,
            market_binding: read_id(&mut r)?,
            source_admission_node: read_id(&mut r)?,
            selected_feed: read_id(&mut r)?,
            order_set: read_id(&mut r)?,
            economic_domain_digest: read_id(&mut r)?,
            candidate_bundle_digest: read_id(&mut r)?,
            settlement_candidate_id: read_id(&mut r)?,
            base_relation_candidate_id: read_id(&mut r)?,
            settlement_witness_digest: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            price_measure_policy_v1_id: read_id(&mut r)?,
            native_claim_basis_id: read_id(&mut r)?,
            candidate_price_digest: read_id(&mut r)?,
            price_body_digest: read_id(&mut r)?,
            score_policy_id: read_id(&mut r)?,
            solver_reward_destination: read_id(&mut r)?,
            rank_key: r.array()?,
            epoch_generation: r.u64()?,
            ordinal: r.u64()?,
            selected_slot: r.u64()?,
            slice_count: r.u16()?,
            next_slice_index: r.u16()?,
            rent: read_rent(&mut r)?,
            candidate_kind: SettlementCandidateKindV1::from_byte(r.u8()?)?,
            price_witness_schema: r.u8()?,
            quantized_semantics_version: r.u8()?,
            rank_key_len: r.u8()?,
            entitlement_state: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Admission-node lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdmissionNodeStatusV1 {
    /// Commitment exists but no bundle has been opened.
    Committed = 0,
    /// Bundle opened and funded verification has not reached a verdict.
    Revealed = 1,
    /// Checked verdict is valid and rank bytes are present.
    VerifiedValid = 2,
    /// Checked verdict refused the candidate.
    VerifiedRefused = 3,
    /// Commitment was never opened before its deadline.
    ExpiredCommitment = 4,
    /// Opened bundle was not verified before its deadline.
    ExpiredUnverified = 5,
}

impl AdmissionNodeStatusV1 {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Committed => 0,
            Self::Revealed => 1,
            Self::VerifiedValid => 2,
            Self::VerifiedRefused => 3,
            Self::ExpiredCommitment => 4,
            Self::ExpiredUnverified => 5,
        }
    }

    fn from_byte(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Committed),
            1 => Ok(Self::Revealed),
            2 => Ok(Self::VerifiedValid),
            3 => Ok(Self::VerifiedRefused),
            4 => Ok(Self::ExpiredCommitment),
            5 => Ok(Self::ExpiredUnverified),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Individually funded candidate-bundle anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionNodeV3AccountV1 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Relation policy.
    pub relation_policy_id: Id32,
    /// This node PDA.
    pub node: Id32,
    /// Prior live reverse-list head.
    pub previous_node: Id32,
    /// Admission policy.
    pub admission_policy_id: Id32,
    /// Score policy.
    pub score_policy_id: Id32,
    /// Domain-separated commitment.
    pub commitment: Id32,
    /// Required reveal authority.
    pub submitter_authority: Id32,
    /// Immutable solver reward destination.
    pub solver_reward_destination: Id32,
    /// Exact funding payer.
    pub payer: Id32,
    /// Exact refund destination.
    pub refund_destination: Id32,
    /// Commitment-opened bundle digest.
    pub candidate_bundle_digest: Id32,
    /// Typed final ID used by score, verdict, cleanup, and settlement.
    pub settlement_candidate_id: Id32,
    /// Verified base RelationV2 candidate digest.
    pub base_relation_candidate_id: Id32,
    /// Exact settlement allocation-witness digest.
    pub settlement_witness_digest: Id32,
    /// Canonical 88-byte active rank plus zero padding.
    pub rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Window-assigned one-based ordinal.
    pub ordinal: u64,
    /// Commit slot.
    pub committed_slot: u64,
    /// Window freeze slot.
    pub window_frozen_slot: u64,
    /// Reveal slot.
    pub revealed_slot: u64,
    /// Verdict or expiry slot.
    pub terminal_slot: u64,
    /// Disjoint node rent owner, principal, and hostile prefund floor.
    pub rent: DeletableRentOwnerV1,
    /// Exact bond.
    pub bond_lamports: u64,
    /// Prepaid node close reward.
    pub cleanup_reward: u64,
    /// Work principal/reward escrow not yet moved into ClearWork.
    pub work_escrow_lamports: u64,
    /// Initial exact work capitalization, preserved for audit.
    pub work_funding_initial: u64,
    /// Must equal 88 only for valid status; zero otherwise.
    pub rank_key_len: u8,
    /// Direct or CoveredDealer.
    pub candidate_kind: SettlementCandidateKindV1,
    /// Lifecycle status.
    pub status: AdmissionNodeStatusV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl AdmissionNodeV3AccountV1 {
    /// Validate canonical status-dependent fields and alias safety.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.node,
            self.admission_policy_id,
            self.score_policy_id,
            self.commitment,
            self.submitter_authority,
            self.solver_reward_destination,
            self.payer,
            self.refund_destination,
        ] {
            live(id)?;
        }
        if self.epoch_generation == 0
            || self.ordinal == 0
            || self.committed_slot == 0
            || self.window_frozen_slot == 0
            || self.bond_lamports == 0
            || self.cleanup_reward == 0
            || self.flags != 0
            || [
                self.previous_node,
                self.submitter_authority,
                self.solver_reward_destination,
                self.payer,
                self.refund_destination,
                self.rent.payer,
            ]
            .contains(&self.node)
            || self.rank_key[SCORE_V2_Q_ACTIVE_RANK_BYTES..]
                .iter()
                .any(|b| *b != 0)
        {
            return Err(CodecError::InvalidState);
        }
        self.rent.validate()?;
        let opened = matches!(
            self.status,
            AdmissionNodeStatusV1::Revealed
                | AdmissionNodeStatusV1::VerifiedValid
                | AdmissionNodeStatusV1::VerifiedRefused
                | AdmissionNodeStatusV1::ExpiredUnverified
        );
        if opened {
            for id in [
                self.candidate_bundle_digest,
                self.settlement_candidate_id,
                self.base_relation_candidate_id,
                self.settlement_witness_digest,
            ] {
                live(id)?;
            }
            if self.revealed_slot < self.committed_slot
                || self.work_funding_initial == 0
                || self.work_escrow_lamports > self.work_funding_initial
            {
                return Err(CodecError::InvalidState);
            }
            if self.candidate_kind == SettlementCandidateKindV1::Direct
                && self.settlement_candidate_id != self.base_relation_candidate_id
            {
                return Err(CodecError::MismatchedBinding);
            }
        } else {
            for id in [
                self.candidate_bundle_digest,
                self.settlement_candidate_id,
                self.base_relation_candidate_id,
                self.settlement_witness_digest,
            ] {
                absent(id)?;
            }
            if self.revealed_slot != 0
                || self.work_funding_initial != 0
                || self.work_escrow_lamports != 0
            {
                return Err(CodecError::NonCanonicalPadding);
            }
        }
        if self.status == AdmissionNodeStatusV1::VerifiedValid {
            if usize::from(self.rank_key_len) != SCORE_V2_Q_ACTIVE_RANK_BYTES
                || self.terminal_slot < self.revealed_slot
            {
                return Err(CodecError::InvalidState);
            }
            let final_id = self.settlement_candidate_id.bytes();
            let mut index = 0usize;
            while index < ID_BYTES {
                if self.rank_key[24 + index] != !final_id[index] {
                    return Err(CodecError::MismatchedBinding);
                }
                index += 1;
            }
            let ordinal = FirstAdmittedTieV1 {
                ordinal: self.ordinal,
            }
            .coordinate()?;
            index = 0;
            while index < ID_BYTES {
                if self.rank_key[56 + index] != !ordinal[index] {
                    return Err(CodecError::MismatchedBinding);
                }
                index += 1;
            }
        } else if self.rank_key_len != 0 || self.rank_key != [0u8; SCORE_V2_Q_RANK_CAPACITY] {
            return Err(CodecError::NonCanonicalPadding);
        }
        match self.status {
            AdmissionNodeStatusV1::Committed | AdmissionNodeStatusV1::Revealed => {
                if self.terminal_slot != 0 {
                    return Err(CodecError::InvalidState);
                }
            }
            AdmissionNodeStatusV1::VerifiedValid
            | AdmissionNodeStatusV1::VerifiedRefused
            | AdmissionNodeStatusV1::ExpiredUnverified => {
                if self.terminal_slot < self.revealed_slot {
                    return Err(CodecError::InvalidState);
                }
            }
            AdmissionNodeStatusV1::ExpiredCommitment => {
                if self.terminal_slot < self.committed_slot {
                    return Err(CodecError::InvalidState);
                }
            }
        }
        Ok(())
    }

    /// Encode exactly [`ADMISSION_NODE_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, ADMISSION_NODE_ACCOUNT_BYTES)?;
        header(
            &mut w,
            ADMISSION_NODE_ACCOUNT_TAG,
            ADMISSION_NODE_ACCOUNT_VERSION,
        )?;
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.node,
            self.previous_node,
            self.admission_policy_id,
            self.score_policy_id,
            self.commitment,
            self.submitter_authority,
            self.solver_reward_destination,
            self.payer,
            self.refund_destination,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.settlement_witness_digest,
        ] {
            w.bytes(&id.bytes())?;
        }
        w.bytes(&self.rank_key)?;
        for value in [
            self.epoch_generation,
            self.ordinal,
            self.committed_slot,
            self.window_frozen_slot,
            self.revealed_slot,
            self.terminal_slot,
            self.bond_lamports,
            self.cleanup_reward,
            self.work_escrow_lamports,
            self.work_funding_initial,
        ] {
            w.u64(value)?;
        }
        write_rent(&mut w, self.rent)?;
        w.u8(self.rank_key_len)?;
        w.u8(self.candidate_kind.to_byte())?;
        w.u8(self.status.to_byte())?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode and validate exactly [`ADMISSION_NODE_ACCOUNT_BYTES`] hostile
    /// bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, ADMISSION_NODE_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            ADMISSION_NODE_ACCOUNT_TAG,
            ADMISSION_NODE_ACCOUNT_VERSION,
        )?;
        let value = Self {
            epoch: read_id(&mut r)?,
            market: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            node: read_id(&mut r)?,
            previous_node: Id32::from_bytes(r.array()?),
            admission_policy_id: read_id(&mut r)?,
            score_policy_id: read_id(&mut r)?,
            commitment: read_id(&mut r)?,
            submitter_authority: read_id(&mut r)?,
            solver_reward_destination: read_id(&mut r)?,
            payer: read_id(&mut r)?,
            refund_destination: read_id(&mut r)?,
            candidate_bundle_digest: Id32::from_bytes(r.array()?),
            settlement_candidate_id: Id32::from_bytes(r.array()?),
            base_relation_candidate_id: Id32::from_bytes(r.array()?),
            settlement_witness_digest: Id32::from_bytes(r.array()?),
            rank_key: r.array()?,
            epoch_generation: r.u64()?,
            ordinal: r.u64()?,
            committed_slot: r.u64()?,
            window_frozen_slot: r.u64()?,
            revealed_slot: r.u64()?,
            terminal_slot: r.u64()?,
            bond_lamports: r.u64()?,
            cleanup_reward: r.u64()?,
            work_escrow_lamports: r.u64()?,
            work_funding_initial: r.u64()?,
            rent: read_rent(&mut r)?,
            rank_key_len: r.u8()?,
            candidate_kind: SettlementCandidateKindV1::from_byte(r.u8()?)?,
            status: AdmissionNodeStatusV1::from_byte(r.u8()?)?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Candidate feed header shared by staging and sealed accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeedHeaderV2 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Admission node.
    pub node: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Frozen order-set identity.
    pub order_set: Id32,
    /// Relation policy.
    pub relation_policy_id: Id32,
    /// SHA-256 identity derived from the canonical EconomicDomainV2 artifact.
    pub economic_domain_digest: Id32,
    /// Authenticated NativeClaimBasisV1 exact-body identity.
    pub native_claim_basis_id: Id32,
    /// Canonical price-semantics digest checked by the certificate.
    pub candidate_price_digest: Id32,
    /// Price-measure policy.
    pub price_measure_policy_v1_id: Id32,
    /// Typed claimed final candidate ID.
    pub settlement_candidate_id: Id32,
    /// Claimed base RelationV2 candidate ID.
    pub base_relation_candidate_id: Id32,
    /// Canonical settlement witness digest.
    pub settlement_witness_digest: Id32,
    /// Canonical quantized-certificate body digest.
    pub price_body_digest: Id32,
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Virtual complete-set split.
    pub virtual_split: u64,
    /// Virtual complete-set merge.
    pub virtual_merge: u64,
    /// Exact honored-AON mask.
    pub honored_aon_mask: u64,
    /// Integer simplex scale.
    pub price_scale: u64,
    /// Primitive atom mass denominator.
    pub common_denominator: u64,
    /// Prepaid permissionless feed-close reward.
    pub close_reward_lamports: u64,
    /// Quantized V3 basis degree, zero through three.
    pub basis_degree: u8,
    /// Active outcome width.
    pub outcome_count: u8,
    /// Active order width.
    pub order_count: u8,
    /// Active atom width.
    pub atom_count: u8,
    /// Active settlement-slice width.
    pub slice_count: u16,
    /// Sequential price cursor.
    pub prices_written: u8,
    /// Sequential fill cursor.
    pub fills_written: u8,
    /// Sequential atom cursor.
    pub atoms_written: u8,
    /// Sequential slice cursor.
    pub slices_written: u16,
    /// Direct or CoveredDealer.
    pub candidate_kind: SettlementCandidateKindV1,
    /// Must equal V3.
    pub price_witness_schema: u8,
    /// Must equal quantized integer-grid semantics V1.
    pub quantized_semantics_version: u8,
    /// Feed account rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl CandidateFeedHeaderV2 {
    /// Validate dimensions, bindings, cursors, and rent.
    pub fn validate(self, sealed: bool) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.node,
            self.market,
            self.order_set,
            self.relation_policy_id,
            self.economic_domain_digest,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_measure_policy_v1_id,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.settlement_witness_digest,
            self.price_body_digest,
        ] {
            live(id)?;
        }
        self.rent.validate()?;
        if self.epoch_generation == 0
            || self.price_scale == 0
            || self.common_denominator == 0
            || self.close_reward_lamports == 0
            || self.basis_degree > 3
            || !(2..=MAX_OUTCOMES_U8).contains(&self.outcome_count)
            || self.outcome_count <= self.basis_degree
            || self.order_count > MAX_ORDERS_U8
            || self.atom_count == 0
            || self.atom_count > self.outcome_count
            || self.atom_count > MAX_QUANTIZED_ATOMS_U8
            || self.slice_count > MAX_SLICES_U16
            || self.prices_written > self.outcome_count
            || self.fills_written > self.order_count
            || self.atoms_written > self.atom_count
            || self.slices_written > self.slice_count
            || self.flags != 0
            || self.price_witness_schema != PRICE_MEASURE_WITNESS_SCHEMA_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_V1
            || (self.virtual_split != 0 && self.virtual_merge != 0)
            || (self.order_count < 64 && (self.honored_aon_mask >> self.order_count) != 0)
        {
            return Err(CodecError::InvalidState);
        }
        if self.candidate_kind == SettlementCandidateKindV1::Direct
            && self.settlement_candidate_id != self.base_relation_candidate_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        if sealed
            && (self.prices_written != self.outcome_count
                || self.fills_written != self.order_count
                || self.atoms_written != self.atom_count
                || self.slices_written != self.slice_count)
        {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode only the exact [`CANDIDATE_FEED_HEADER_BYTES`] header, using the
    /// sealed or stage tag.
    pub fn encode(self, out: &mut [u8], sealed: bool) -> Result<(), CodecError> {
        self.validate(sealed)?;
        let mut w = Writer::exact(out, CANDIDATE_FEED_HEADER_BYTES)?;
        if sealed {
            header(
                &mut w,
                CANDIDATE_FEED_ACCOUNT_TAG,
                CANDIDATE_FEED_ACCOUNT_VERSION,
            )?;
        } else {
            header(
                &mut w,
                CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
                CANDIDATE_FEED_STAGE_ACCOUNT_VERSION,
            )?;
        }
        for id in [
            self.epoch,
            self.node,
            self.market,
            self.order_set,
            self.relation_policy_id,
            self.economic_domain_digest,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_measure_policy_v1_id,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.settlement_witness_digest,
            self.price_body_digest,
        ] {
            w.bytes(&id.bytes())?;
        }
        for value in [
            self.epoch_generation,
            self.virtual_split,
            self.virtual_merge,
            self.honored_aon_mask,
            self.price_scale,
            self.common_denominator,
            self.close_reward_lamports,
        ] {
            w.u64(value)?;
        }
        for value in [
            self.basis_degree,
            self.outcome_count,
            self.order_count,
            self.atom_count,
        ] {
            w.u8(value)?;
        }
        w.u16(self.slice_count)?;
        w.u8(self.prices_written)?;
        w.u8(self.fills_written)?;
        w.u8(self.atoms_written)?;
        w.u16(self.slices_written)?;
        w.u8(self.candidate_kind.to_byte())?;
        w.u8(self.price_witness_schema)?;
        w.u8(self.quantized_semantics_version)?;
        write_rent(&mut w, self.rent)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.finish()
    }

    /// Decode a header and require exactly the expected full active frame.
    pub fn decode_account(input: &[u8], sealed: bool) -> Result<Self, CodecError> {
        if input.len() < CANDIDATE_FEED_HEADER_BYTES {
            return Err(CodecError::WrongLength);
        }
        let header_bytes = &input[..CANDIDATE_FEED_HEADER_BYTES];
        let mut r = Reader::exact(header_bytes, CANDIDATE_FEED_HEADER_BYTES)?;
        if sealed {
            check_header(
                &mut r,
                CANDIDATE_FEED_ACCOUNT_TAG,
                CANDIDATE_FEED_ACCOUNT_VERSION,
            )?;
        } else {
            check_header(
                &mut r,
                CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
                CANDIDATE_FEED_STAGE_ACCOUNT_VERSION,
            )?;
        }
        let value = Self {
            epoch: read_id(&mut r)?,
            node: read_id(&mut r)?,
            market: read_id(&mut r)?,
            order_set: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            economic_domain_digest: read_id(&mut r)?,
            native_claim_basis_id: read_id(&mut r)?,
            candidate_price_digest: read_id(&mut r)?,
            price_measure_policy_v1_id: read_id(&mut r)?,
            settlement_candidate_id: read_id(&mut r)?,
            base_relation_candidate_id: read_id(&mut r)?,
            settlement_witness_digest: read_id(&mut r)?,
            price_body_digest: read_id(&mut r)?,
            epoch_generation: r.u64()?,
            virtual_split: r.u64()?,
            virtual_merge: r.u64()?,
            honored_aon_mask: r.u64()?,
            price_scale: r.u64()?,
            common_denominator: r.u64()?,
            close_reward_lamports: r.u64()?,
            basis_degree: r.u8()?,
            outcome_count: r.u8()?,
            order_count: r.u8()?,
            atom_count: r.u8()?,
            slice_count: r.u16()?,
            prices_written: r.u8()?,
            fills_written: r.u8()?,
            atoms_written: r.u8()?,
            slices_written: r.u16()?,
            candidate_kind: SettlementCandidateKindV1::from_byte(r.u8()?)?,
            price_witness_schema: r.u8()?,
            quantized_semantics_version: r.u8()?,
            rent: read_rent(&mut r)?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate(sealed)?;
        if input.len()
            != candidate_feed_account_len(
                value.outcome_count,
                value.order_count,
                value.atom_count,
                value.slice_count,
            )?
        {
            return Err(CodecError::WrongLength);
        }
        validate_candidate_feed_tail(input, value, sealed)?;
        Ok(value)
    }
}

/// Compute the exact active-width feed or feed-stage length.
pub fn candidate_feed_account_len(
    outcomes: u8,
    orders: u8,
    atoms: u8,
    slices: u16,
) -> Result<usize, CodecError> {
    if !(2..=MAX_OUTCOMES_U8).contains(&outcomes)
        || orders > MAX_ORDERS_U8
        || atoms == 0
        || atoms > outcomes
        || atoms > MAX_QUANTIZED_ATOMS_U8
        || slices > MAX_SLICES_U16
    {
        return Err(CodecError::InvalidCount);
    }
    CANDIDATE_FEED_HEADER_BYTES
        .checked_add(usize::from(outcomes) * 8)
        .and_then(|v| v.checked_add(usize::from(orders) * 8))
        .and_then(|v| v.checked_add(usize::from(atoms) * QUANTIZED_ATOM_BYTES))
        .and_then(|v| v.checked_add(usize::from(slices) * SETTLEMENT_SLICE_BYTES))
        .ok_or(CodecError::ArithmeticOverflow)
}

/// Borrowed active-width tails of one CandidateFeed or FeedStage.
///
/// This is an offset projection, not an authenticated account capability. It
/// can be obtained only after exact frame geometry is checked, so adapters do
/// not need to restate active-tail offsets or record widths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeedTailV2<'a> {
    prices_le: &'a [u8],
    fills_le: &'a [u8],
    atoms_le: &'a [u8],
    slices_le: &'a [u8],
}

/// Canonical role of one leg in a thirteen-byte settlement slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementSliceLegKindV1 {
    /// One exact active order index.
    Order = 0,
    /// The covered dealer supplies Eggs to a user buy; valid only as the sell leg.
    CoveredDealerSell = 1,
    /// The covered dealer receives Eggs from a user sell; valid only as the buy leg.
    CoveredDealerBuy = 2,
}

/// Decoded canonical settlement-slice record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementSliceV1 {
    /// Buy-leg kind.
    pub buy_kind: SettlementSliceLegKindV1,
    /// Buy order index, or zero for a covered-dealer leg.
    pub buy_index: u8,
    /// Sell-leg kind.
    pub sell_kind: SettlementSliceLegKindV1,
    /// Sell order index, or zero for a covered-dealer leg.
    pub sell_index: u8,
    /// Active native outcome transferred by this slice.
    pub outcome: u8,
    /// Positive native Egg quantity.
    pub quantity: u64,
}

impl SettlementSliceV1 {
    /// Decode one exact record under its authenticated active widths.
    pub fn decode(input: &[u8], order_count: u8, outcome_count: u8) -> Result<Self, CodecError> {
        if input.len() != SETTLEMENT_SLICE_BYTES {
            return Err(CodecError::WrongLength);
        }
        let buy_kind = match input[0] {
            0 => SettlementSliceLegKindV1::Order,
            2 => SettlementSliceLegKindV1::CoveredDealerBuy,
            _ => return Err(CodecError::InvalidState),
        };
        let sell_kind = match input[2] {
            0 => SettlementSliceLegKindV1::Order,
            1 => SettlementSliceLegKindV1::CoveredDealerSell,
            _ => return Err(CodecError::InvalidState),
        };
        if (buy_kind == SettlementSliceLegKindV1::Order && input[1] >= order_count)
            || (buy_kind != SettlementSliceLegKindV1::Order && input[1] != 0)
            || (sell_kind == SettlementSliceLegKindV1::Order && input[3] >= order_count)
            || (sell_kind != SettlementSliceLegKindV1::Order && input[3] != 0)
            || input[4] >= outcome_count
            || (buy_kind != SettlementSliceLegKindV1::Order
                && sell_kind != SettlementSliceLegKindV1::Order)
        {
            return Err(CodecError::InvalidState);
        }
        let mut quantity = [0u8; 8];
        quantity.copy_from_slice(&input[5..13]);
        let quantity = u64::from_le_bytes(quantity);
        if quantity == 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(Self {
            buy_kind,
            buy_index: input[1],
            sell_kind,
            sell_index: input[3],
            outcome: input[4],
            quantity,
        })
    }
}

/// Exact byte boundaries of every active-width CandidateFeed tail.
///
/// Keeping this projection beside the frame codec prevents adapters from
/// independently reconstructing tail offsets when copying segment records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeedTailOffsetsV2 {
    prices_at: usize,
    fills_at: usize,
    atoms_at: usize,
    slices_at: usize,
    end: usize,
}

impl CandidateFeedTailOffsetsV2 {
    /// First active price byte.
    pub const fn prices_at(self) -> usize {
        self.prices_at
    }

    /// First active fill byte.
    pub const fn fills_at(self) -> usize {
        self.fills_at
    }

    /// First active quantized-atom byte.
    pub const fn atoms_at(self) -> usize {
        self.atoms_at
    }

    /// First active settlement-slice byte.
    pub const fn slices_at(self) -> usize {
        self.slices_at
    }

    /// Exact end of the active frame.
    pub const fn end(self) -> usize {
        self.end
    }
}

impl<'a> CandidateFeedTailV2<'a> {
    /// Exact active price records, each one little-endian `u64`.
    pub const fn prices_le(self) -> &'a [u8] {
        self.prices_le
    }

    /// Exact active fill records, each one little-endian `u64`.
    pub const fn fills_le(self) -> &'a [u8] {
        self.fills_le
    }

    /// Exact active atom records, each `u128 coordinate || u64 mass`, LE.
    pub const fn atoms_le(self) -> &'a [u8] {
        self.atoms_le
    }

    /// Exact active 13-byte settlement-slice records.
    pub const fn slices_le(self) -> &'a [u8] {
        self.slices_le
    }
}

/// Project exact active-tail byte boundaries from a validated header.
pub fn candidate_feed_tail_offsets_v2(
    header: CandidateFeedHeaderV2,
) -> Result<CandidateFeedTailOffsetsV2, CodecError> {
    header.validate(false)?;
    let prices_bytes = usize::from(header.outcome_count)
        .checked_mul(8)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let fills_bytes = usize::from(header.order_count)
        .checked_mul(8)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let atoms_bytes = usize::from(header.atom_count)
        .checked_mul(QUANTIZED_ATOM_BYTES)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let prices_at = CANDIDATE_FEED_HEADER_BYTES;
    let fills_at = prices_at
        .checked_add(prices_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let atoms_at = fills_at
        .checked_add(fills_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let slices_at = atoms_at
        .checked_add(atoms_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let slices_bytes = usize::from(header.slice_count)
        .checked_mul(SETTLEMENT_SLICE_BYTES)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let end = slices_at
        .checked_add(slices_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    Ok(CandidateFeedTailOffsetsV2 {
        prices_at,
        fills_at,
        atoms_at,
        slices_at,
        end,
    })
}

/// Project exact active tails after checking complete frame geometry.
pub fn candidate_feed_tail_v2<'a>(
    input: &'a [u8],
    header: CandidateFeedHeaderV2,
) -> Result<CandidateFeedTailV2<'a>, CodecError> {
    let exact = candidate_feed_account_len(
        header.outcome_count,
        header.order_count,
        header.atom_count,
        header.slice_count,
    )?;
    if input.len() != exact {
        return Err(CodecError::WrongLength);
    }
    let offsets = candidate_feed_tail_offsets_v2(header)?;
    Ok(CandidateFeedTailV2 {
        prices_le: input
            .get(offsets.prices_at()..offsets.fills_at())
            .ok_or(CodecError::WrongLength)?,
        fills_le: input
            .get(offsets.fills_at()..offsets.atoms_at())
            .ok_or(CodecError::WrongLength)?,
        atoms_le: input
            .get(offsets.atoms_at()..offsets.slices_at())
            .ok_or(CodecError::WrongLength)?,
        slices_le: input
            .get(offsets.slices_at()..offsets.end())
            .ok_or(CodecError::WrongLength)?,
    })
}

/// Decode an exact Feed/Stage, require every active cursor complete, and apply
/// sealed simplex/atom/slice semantics even before an atomic Stage tag flip.
pub fn complete_candidate_feed_v2(
    input: &[u8],
    sealed: bool,
) -> Result<(CandidateFeedHeaderV2, CandidateFeedTailV2<'_>), CodecError> {
    let header = CandidateFeedHeaderV2::decode_account(input, sealed)?;
    if header.prices_written != header.outcome_count
        || header.fills_written != header.order_count
        || header.atoms_written != header.atom_count
        || header.slices_written != header.slice_count
    {
        return Err(CodecError::InvalidState);
    }
    // Apply sealed semantic validation even when the bytes still carry the
    // stage discriminator immediately before an atomic seal transition.
    header.validate(true)?;
    validate_candidate_feed_tail(input, header, true)?;
    let tail = candidate_feed_tail_v2(input, header)?;
    Ok((header, tail))
}

/// Recompute the canonical V3 quantized witness-body digest from a complete
/// active-width Feed/Stage. The stored claimed body digest is excluded.
pub fn quantized_witness_body_digest_v3<B: Sha256BackendV1>(
    backend: &B,
    candidate_feed: Id32,
    input: &[u8],
    sealed: bool,
) -> Result<Id32, CodecError> {
    let (header, tail) = complete_candidate_feed_v2(input, sealed)?;
    quantized_witness_parts_digest_v3(
        backend,
        candidate_feed,
        header.economic_domain_digest,
        header.native_claim_basis_id,
        header.candidate_price_digest,
        header.price_scale,
        header.common_denominator,
        header.price_witness_schema,
        header.quantized_semantics_version,
        header.basis_degree,
        header.outcome_count,
        header.atom_count,
        tail.prices_le(),
        tail.atoms_le(),
    )
}

/// Derive the canonical V3 quantized witness-body digest from checked typed
/// fields and exact active-width price/atom tails.
///
/// This is the single transcript owner used both before a feed is serialized
/// by an offchain builder and after a Feed/Stage is decoded onchain. Callers
/// must supply little-endian `u64` prices and `(u128, u64)` atom records with
/// no inactive padding.
#[allow(clippy::too_many_arguments)]
pub fn quantized_witness_parts_digest_v3<B: Sha256BackendV1>(
    backend: &B,
    candidate_feed: Id32,
    economic_domain_digest: Id32,
    native_claim_basis_id: Id32,
    candidate_price_digest: Id32,
    price_scale: u64,
    common_denominator: u64,
    price_witness_schema: u8,
    quantized_semantics_version: u8,
    basis_degree: u8,
    outcome_count: u8,
    atom_count: u8,
    prices_le: &[u8],
    atoms_le: &[u8],
) -> Result<Id32, CodecError> {
    for id in [
        candidate_feed,
        economic_domain_digest,
        native_claim_basis_id,
        candidate_price_digest,
    ] {
        live(id)?;
    }
    let expected_prices = usize::from(outcome_count)
        .checked_mul(8)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let expected_atoms = usize::from(atom_count)
        .checked_mul(QUANTIZED_ATOM_BYTES)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if price_scale == 0
        || common_denominator == 0
        || price_witness_schema != PRICE_MEASURE_WITNESS_SCHEMA_V3
        || quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_V1
        || basis_degree > 3
        || !(2..=MAX_OUTCOMES_U8).contains(&outcome_count)
        || outcome_count <= basis_degree
        || atom_count == 0
        || atom_count > outcome_count
        || atom_count > MAX_QUANTIZED_ATOMS_U8
        || prices_le.len() != expected_prices
        || atoms_le.len() != expected_atoms
    {
        return Err(CodecError::InvalidState);
    }
    let mut fixed = [0u8; QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES];
    let mut w = Writer::exact(&mut fixed, QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES)?;
    for id in [
        candidate_feed,
        economic_domain_digest,
        native_claim_basis_id,
        candidate_price_digest,
    ] {
        w.bytes(&id.bytes())?;
    }
    w.u64(price_scale)?;
    w.u64(common_denominator)?;
    w.u8(price_witness_schema)?;
    w.u8(quantized_semantics_version)?;
    w.u8(basis_degree)?;
    w.u8(outcome_count)?;
    w.u8(atom_count)?;
    w.finish()?;
    Id32::new(backend.sha256(&[
        QUANTIZED_WITNESS_BODY_DIGEST_DOMAIN_V3,
        &fixed,
        prices_le,
        atoms_le,
    ]))
}

/// Recompute the canonical settlement-witness identity from exact slice bytes.
pub fn settlement_witness_digest_v1<B: Sha256BackendV1>(
    backend: &B,
    base_relation_candidate_id: Id32,
    slice_count: u16,
    encoded_slices: &[u8],
) -> Result<Id32, CodecError> {
    live(base_relation_candidate_id)?;
    let expected = usize::from(slice_count)
        .checked_mul(SETTLEMENT_SLICE_BYTES)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if slice_count > MAX_SLICES_U16 || encoded_slices.len() != expected {
        return Err(CodecError::WrongLength);
    }
    let count = slice_count.to_le_bytes();
    Id32::new(backend.sha256(&[
        SETTLEMENT_WITNESS_DIGEST_DOMAIN_V1,
        &base_relation_candidate_id.bytes(),
        &count,
        encoded_slices,
    ]))
}

/// Recompute the canonical nonzero empty settlement-witness identity.
pub fn empty_settlement_witness_digest_v1<B: Sha256BackendV1>(
    backend: &B,
    base_relation_candidate_id: Id32,
) -> Result<Id32, CodecError> {
    settlement_witness_digest_v1(backend, base_relation_candidate_id, 0, &[])
}

/// Recompute one complete General V2 candidate-bundle identity from typed
/// header fields and exact active tails. RelationV2 remains the sole owner of
/// `base_relation_candidate_id`; this transcript only binds that checked ID.
pub fn candidate_bundle_digest_v1<B: Sha256BackendV1>(
    backend: &B,
    input: &[u8],
    sealed: bool,
) -> Result<Id32, CodecError> {
    let (header, tail) = complete_candidate_feed_v2(input, sealed)?;
    let mut fixed = [0u8; CANDIDATE_BUNDLE_V1_FIXED_BYTES];
    let mut w = Writer::exact(&mut fixed, CANDIDATE_BUNDLE_V1_FIXED_BYTES)?;
    for id in [
        header.order_set,
        header.relation_policy_id,
        header.economic_domain_digest,
        header.native_claim_basis_id,
        header.candidate_price_digest,
        header.price_measure_policy_v1_id,
        header.settlement_candidate_id,
        header.base_relation_candidate_id,
        header.settlement_witness_digest,
        header.price_body_digest,
    ] {
        w.bytes(&id.bytes())?;
    }
    for value in [
        header.virtual_split,
        header.virtual_merge,
        header.honored_aon_mask,
        header.price_scale,
        header.common_denominator,
    ] {
        w.u64(value)?;
    }
    w.u16(header.slice_count)?;
    w.u8(header.basis_degree)?;
    w.u8(header.outcome_count)?;
    w.u8(header.order_count)?;
    w.u8(header.atom_count)?;
    w.u8(header.candidate_kind.to_byte())?;
    w.u8(header.price_witness_schema)?;
    w.u8(header.quantized_semantics_version)?;
    w.finish()?;
    Id32::new(backend.sha256(&[
        CANDIDATE_BUNDLE_DIGEST_DOMAIN_V1,
        &fixed,
        tail.prices_le(),
        tail.fills_le(),
        tail.atoms_le(),
        tail.slices_le(),
    ]))
}

fn validate_candidate_feed_tail(
    input: &[u8],
    header: CandidateFeedHeaderV2,
    sealed: bool,
) -> Result<(), CodecError> {
    let prices_bytes = usize::from(header.outcome_count)
        .checked_mul(8)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let fills_bytes = usize::from(header.order_count)
        .checked_mul(8)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let atoms_bytes = usize::from(header.atom_count)
        .checked_mul(QUANTIZED_ATOM_BYTES)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let prices_at = CANDIDATE_FEED_HEADER_BYTES;
    let fills_at = prices_at
        .checked_add(prices_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let atoms_at = fills_at
        .checked_add(fills_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let slices_at = atoms_at
        .checked_add(atoms_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;

    let mut price_sum = 0u64;
    let mut index = 0u8;
    while index < header.outcome_count {
        let value = read_u64_at(input, prices_at + (usize::from(index) * 8))?;
        if index >= header.prices_written && value != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        if index < header.prices_written {
            price_sum = price_sum
                .checked_add(value)
                .ok_or(CodecError::ArithmeticOverflow)?;
        }
        index += 1;
    }
    if sealed && price_sum != header.price_scale {
        return Err(CodecError::MismatchedBinding);
    }

    index = header.fills_written;
    while index < header.order_count {
        if read_u64_at(input, fills_at + (usize::from(index) * 8))? != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        index += 1;
    }

    let mut prior_coordinate = 0u128;
    let mut mass_sum = 0u64;
    let mut mass_divisor = header.common_denominator;
    index = 0;
    while index < header.atom_count {
        let at = atoms_at + (usize::from(index) * QUANTIZED_ATOM_BYTES);
        let coordinate = read_u128_at(input, at)?;
        let mass = read_u64_at(input, at + 16)?;
        if index >= header.atoms_written {
            if coordinate != 0 || mass != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else {
            if mass == 0 || (index != 0 && coordinate <= prior_coordinate) {
                return Err(CodecError::InvalidState);
            }
            prior_coordinate = coordinate;
            mass_sum = mass_sum
                .checked_add(mass)
                .ok_or(CodecError::ArithmeticOverflow)?;
            mass_divisor = gcd(mass_divisor, mass);
        }
        index += 1;
    }
    if sealed && (mass_sum != header.common_denominator || mass_divisor != 1) {
        return Err(CodecError::MismatchedBinding);
    }

    let mut slice = 0u16;
    while slice < header.slice_count {
        let at = slices_at + (usize::from(slice) * SETTLEMENT_SLICE_BYTES);
        if slice >= header.slices_written {
            if input
                .get(at..at + SETTLEMENT_SLICE_BYTES)
                .ok_or(CodecError::WrongLength)?
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else {
            validate_slice(input, at, header.order_count, header.outcome_count)?;
        }
        slice += 1;
    }
    Ok(())
}

fn validate_slice(
    input: &[u8],
    at: usize,
    order_count: u8,
    outcome_count: u8,
) -> Result<(), CodecError> {
    let bytes = input
        .get(at..at + SETTLEMENT_SLICE_BYTES)
        .ok_or(CodecError::WrongLength)?;
    SettlementSliceV1::decode(bytes, order_count, outcome_count).map(|_| ())
}

fn read_u64_at(input: &[u8], at: usize) -> Result<u64, CodecError> {
    let end = at.checked_add(8).ok_or(CodecError::ArithmeticOverflow)?;
    let source = input.get(at..end).ok_or(CodecError::WrongLength)?;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(source);
    Ok(u64::from_le_bytes(bytes))
}

fn read_u128_at(input: &[u8], at: usize) -> Result<u128, CodecError> {
    let end = at.checked_add(16).ok_or(CodecError::ArithmeticOverflow)?;
    let source = input.get(at..end).ok_or(CodecError::WrongLength)?;
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(source);
    Ok(u128::from_le_bytes(bytes))
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Serializable SHA-256 continuation state. It is a work checkpoint, not a
/// semantic digest owner. Native one-shot and portable paths must match it in
/// differential vectors before SBF activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256CheckpointV1 {
    /// Eight SHA-256 chaining words.
    pub state: [u32; 8],
    /// Pending partial block.
    pub block: [u8; 64],
    /// Active bytes in `block`.
    pub block_len: u8,
    /// Total transcript bytes consumed.
    pub total_len: u64,
}

impl Sha256CheckpointV1 {
    /// Validate canonical unused block bytes and length congruence.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.block_len >= 64 || self.total_len % 64 != u64::from(self.block_len) {
            return Err(CodecError::InvalidState);
        }
        if self.block[usize::from(self.block_len)..]
            .iter()
            .any(|b| *b != 0)
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        if self.total_len == 0 && self.state != SHA256_INITIAL_STATE_V1 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Project into the canonical RelationV2 stream checkpoint without
    /// changing a byte of continuation state.
    pub fn relation_v2(
        self,
    ) -> Result<clutch_batch::relation_v2::EconomicSha256CheckpointV2, CodecError> {
        self.validate()?;
        let value = clutch_batch::relation_v2::EconomicSha256CheckpointV2 {
            state: self.state,
            block: self.block,
            block_len: self.block_len,
            total_len: self.total_len,
        };
        value.validate().map_err(|_| CodecError::InvalidState)
    }

    /// Persist an exact checkpoint returned by the RelationV2 stream owner.
    pub fn from_relation_v2(
        value: clutch_batch::relation_v2::EconomicSha256CheckpointV2,
    ) -> Result<Self, CodecError> {
        value.validate().map_err(|_| CodecError::InvalidState)?;
        let checkpoint = Self {
            state: value.state,
            block: value.block,
            block_len: value.block_len,
            total_len: value.total_len,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }
}

/// Active-width RelationV2 and settlement verification checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkHeaderV2 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Admission node.
    pub node: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Frozen order set.
    pub order_set: Id32,
    /// Canonical feed account.
    pub feed: Id32,
    /// Commitment-opened bundle digest.
    pub candidate_bundle_digest: Id32,
    /// Typed final candidate ID.
    pub settlement_candidate_id: Id32,
    /// Base RelationV2 candidate ID.
    pub base_relation_candidate_id: Id32,
    /// Relation policy.
    pub relation_policy_id: Id32,
    /// SHA-256 identity derived from the canonical EconomicDomainV2 artifact.
    pub economic_domain_digest: Id32,
    /// Authenticated NativeClaimBasisV1 exact-body identity.
    pub native_claim_basis_id: Id32,
    /// Canonical price-semantics digest checked by the certificate.
    pub candidate_price_digest: Id32,
    /// Price-measure policy.
    pub price_measure_policy_v1_id: Id32,
    /// Score policy.
    pub score_policy_id: Id32,
    /// Authenticated certificate body digest.
    pub price_body_digest: Id32,
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Disjoint work rent owner, principal, and hostile prefund floor.
    pub rent: DeletableRentOwnerV1,
    /// Unpaid work reward reserve.
    pub reward_remaining: u64,
    /// Already paid monotone progress rewards.
    pub reward_earned: u64,
    /// Declared settlement slices.
    pub slice_count: u16,
    /// Next settlement slice.
    pub slice_cursor: u16,
    /// Active outcomes.
    pub outcome_count: u8,
    /// Active orders.
    pub order_count: u8,
    /// Next order.
    pub order_cursor: u8,
    /// Price/orders/slices/complete phase.
    pub phase: u8,
    /// Direct or CoveredDealer.
    pub candidate_kind: SettlementCandidateKindV1,
    /// Must equal V3.
    pub price_witness_schema: u8,
    /// Must equal quantized integer-grid semantics V1.
    pub quantized_semantics_version: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
    /// Resumable canonical RelationV2 digest state.
    pub sha256: Sha256CheckpointV1,
}

impl ClearWorkHeaderV2 {
    /// Validate exact active widths, phase cursors, funding, and SHA state.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.node,
            self.market,
            self.order_set,
            self.feed,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.relation_policy_id,
            self.economic_domain_digest,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_measure_policy_v1_id,
            self.score_policy_id,
            self.price_body_digest,
        ] {
            live(id)?;
        }
        if self.epoch_generation == 0
            || self
                .reward_remaining
                .checked_add(self.reward_earned)
                .is_none()
            || !(2..=MAX_OUTCOMES_U8).contains(&self.outcome_count)
            || self.order_count > MAX_ORDERS_U8
            || self.slice_count > MAX_SLICES_U16
            || self.order_cursor > self.order_count
            || self.slice_cursor > self.slice_count
            || self.phase > 3
            || self.price_witness_schema != PRICE_MEASURE_WITNESS_SCHEMA_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_V1
            || (self.phase == 0 && (self.order_cursor != 0 || self.slice_cursor != 0))
            || (self.phase == 1 && self.slice_cursor != 0)
            || (self.phase >= 2 && self.order_cursor != self.order_count)
            || (self.phase == 3 && self.slice_cursor != self.slice_count)
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        self.rent.validate()?;
        if self.candidate_kind == SettlementCandidateKindV1::Direct
            && self.settlement_candidate_id != self.base_relation_candidate_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        self.sha256.validate()
    }

    /// Encode the exact fixed header. The caller owns the active-width tail
    /// whose total length is returned by [`clear_work_account_len`].
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, CLEAR_WORK_HEADER_BYTES)?;
        header(&mut w, CLEAR_WORK_ACCOUNT_TAG, CLEAR_WORK_ACCOUNT_VERSION)?;
        for id in [
            self.epoch,
            self.node,
            self.market,
            self.order_set,
            self.feed,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.relation_policy_id,
            self.economic_domain_digest,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_measure_policy_v1_id,
            self.score_policy_id,
            self.price_body_digest,
        ] {
            w.bytes(&id.bytes())?;
        }
        for value in [
            self.epoch_generation,
            self.reward_remaining,
            self.reward_earned,
        ] {
            w.u64(value)?;
        }
        write_rent(&mut w, self.rent)?;
        w.u16(self.slice_count)?;
        w.u16(self.slice_cursor)?;
        for value in [
            self.outcome_count,
            self.order_count,
            self.order_cursor,
            self.phase,
            self.candidate_kind.to_byte(),
            self.price_witness_schema,
            self.quantized_semantics_version,
            self.stored_bump,
            self.flags,
        ] {
            w.u8(value)?;
        }
        write_sha(&mut w, self.sha256)?;
        w.finish()
    }

    /// Decode the fixed header and require the full account's exact active length.
    pub fn decode_account(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() < CLEAR_WORK_HEADER_BYTES {
            return Err(CodecError::WrongLength);
        }
        let mut r = Reader::exact(&input[..CLEAR_WORK_HEADER_BYTES], CLEAR_WORK_HEADER_BYTES)?;
        check_header(&mut r, CLEAR_WORK_ACCOUNT_TAG, CLEAR_WORK_ACCOUNT_VERSION)?;
        let value = Self {
            epoch: read_id(&mut r)?,
            node: read_id(&mut r)?,
            market: read_id(&mut r)?,
            order_set: read_id(&mut r)?,
            feed: read_id(&mut r)?,
            candidate_bundle_digest: read_id(&mut r)?,
            settlement_candidate_id: read_id(&mut r)?,
            base_relation_candidate_id: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            economic_domain_digest: read_id(&mut r)?,
            native_claim_basis_id: read_id(&mut r)?,
            candidate_price_digest: read_id(&mut r)?,
            price_measure_policy_v1_id: read_id(&mut r)?,
            score_policy_id: read_id(&mut r)?,
            price_body_digest: read_id(&mut r)?,
            epoch_generation: r.u64()?,
            reward_remaining: r.u64()?,
            reward_earned: r.u64()?,
            rent: read_rent(&mut r)?,
            slice_count: r.u16()?,
            slice_cursor: r.u16()?,
            outcome_count: r.u8()?,
            order_count: r.u8()?,
            order_cursor: r.u8()?,
            phase: r.u8()?,
            candidate_kind: SettlementCandidateKindV1::from_byte(r.u8()?)?,
            price_witness_schema: r.u8()?,
            quantized_semantics_version: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
            sha256: read_sha(&mut r)?,
        };
        r.finish()?;
        value.validate()?;
        if input.len() != clear_work_account_len(value.outcome_count, value.order_count)? {
            return Err(CodecError::WrongLength);
        }
        validate_clear_work_tail(input, value)?;
        Ok(value)
    }
}

/// Compute exact active-width ClearWork length.
pub fn clear_work_account_len(outcomes: u8, orders: u8) -> Result<usize, CodecError> {
    if !(2..=MAX_OUTCOMES_U8).contains(&outcomes) || orders > MAX_ORDERS_U8 {
        return Err(CodecError::InvalidCount);
    }
    CLEAR_WORK_HEADER_BYTES
        .checked_add(usize::from(outcomes) * 16)
        .and_then(|v| v.checked_add(usize::from(outcomes) * usize::from(orders) * 8))
        .ok_or(CodecError::ArithmeticOverflow)
}

fn validate_clear_work_tail(input: &[u8], header: ClearWorkHeaderV2) -> Result<(), CodecError> {
    let accumulators_bytes = usize::from(header.outcome_count)
        .checked_mul(16)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let matrix_at = CLEAR_WORK_HEADER_BYTES
        .checked_add(accumulators_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if header.phase == 0
        && input[CLEAR_WORK_HEADER_BYTES..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(CodecError::NonCanonicalPadding);
    }
    if header.phase == 1 {
        let unprocessed_at = matrix_at
            .checked_add(
                usize::from(header.order_cursor)
                    .checked_mul(usize::from(header.outcome_count))
                    .and_then(|value| value.checked_mul(8))
                    .ok_or(CodecError::ArithmeticOverflow)?,
            )
            .ok_or(CodecError::ArithmeticOverflow)?;
        if input[unprocessed_at..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
    }
    Ok(())
}

/// Checked economic disposition of a resumable RelationV2 Work successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClearWorkVerificationStateV1 {
    /// Page/order or settlement-slice verification remains incomplete.
    Pending,
    /// The complete owner-blind relation accepted the exact submitted candidate.
    Valid,
    /// An authenticated economic or settlement input proved the candidate invalid.
    Refused,
}

impl ClearWorkVerificationStateV1 {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Pending => 0,
            Self::Valid => 1,
            Self::Refused => 2,
        }
    }

    fn from_byte(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Valid),
            2 => Ok(Self::Refused),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Resumable one-page/one-order RelationV2 and settlement checkpoint.
///
/// V3 deliberately does not reinterpret V2. It adds the exact frozen-page
/// cursor, predecessor identity, and checked-refusal latch that a nonempty
/// relation needs while retaining the same active-width flow/leg tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkV3AccountV1 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Admission node.
    pub node: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Frozen order set.
    pub order_set: Id32,
    /// Canonical feed account.
    pub feed: Id32,
    /// Commitment-opened bundle digest.
    pub candidate_bundle_digest: Id32,
    /// Typed final candidate ID.
    pub settlement_candidate_id: Id32,
    /// Base RelationV2 candidate ID.
    pub base_relation_candidate_id: Id32,
    /// Relation policy.
    pub relation_policy_id: Id32,
    /// Canonical EconomicDomainV2 digest.
    pub economic_domain_digest: Id32,
    /// Authenticated NativeClaimBasisV1 identity.
    pub native_claim_basis_id: Id32,
    /// Canonical price-semantics digest.
    pub candidate_price_digest: Id32,
    /// Price-measure policy.
    pub price_measure_policy_v1_id: Id32,
    /// Score policy.
    pub score_policy_id: Id32,
    /// Authenticated certificate body digest.
    pub price_body_digest: Id32,
    /// Last successfully folded live order, absent before the first.
    pub previous_order_id: Id32,
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Disjoint Work rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Unpaid Work reward reserve.
    pub reward_remaining: u64,
    /// Already paid monotone progress rewards.
    pub reward_earned: u64,
    /// Declared settlement slices.
    pub slice_count: u16,
    /// Next settlement slice.
    pub slice_cursor: u16,
    /// Frozen page count, learned from authenticated page zero.
    pub page_count: u16,
    /// Next frozen page.
    pub page_cursor: u16,
    /// Active outcomes.
    pub outcome_count: u8,
    /// Active dense live orders.
    pub order_count: u8,
    /// Next dense live order.
    pub order_cursor: u8,
    /// Next populated slot within `page_cursor`.
    pub slot_cursor: u8,
    /// Idle/orders/slices/complete phase.
    pub phase: u8,
    /// Direct or CoveredDealer.
    pub candidate_kind: SettlementCandidateKindV1,
    /// Must equal V3.
    pub price_witness_schema: u8,
    /// Must equal quantized integer-grid semantics V1.
    pub quantized_semantics_version: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Checked relation disposition.
    pub verification_state: ClearWorkVerificationStateV1,
    /// Reserved zero flags.
    pub flags: u8,
    /// Canonical RelationV2 candidate-identity continuation after live orders.
    pub sha256: Sha256CheckpointV1,
}

impl ClearWorkV3AccountV1 {
    /// Validate exact dimensions, page/order cursors, verdict partition, and funding.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.node,
            self.market,
            self.order_set,
            self.feed,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.relation_policy_id,
            self.economic_domain_digest,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_measure_policy_v1_id,
            self.score_policy_id,
            self.price_body_digest,
        ] {
            live(id)?;
        }
        if self.epoch_generation == 0
            || self
                .reward_remaining
                .checked_add(self.reward_earned)
                .is_none()
            || !(2..=MAX_OUTCOMES_U8).contains(&self.outcome_count)
            || self.order_count > MAX_ORDERS_U8
            || self.slice_count > MAX_SLICES_U16
            || self.order_cursor > self.order_count
            || self.slice_cursor > self.slice_count
            || self.page_count > 4
            || self.page_cursor > self.page_count
            || self.phase > 3
            || self.price_witness_schema != PRICE_MEASURE_WITNESS_SCHEMA_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_V1
            || self.flags != 0
            || (self.order_cursor == 0) != self.previous_order_id.is_zero()
        {
            return Err(CodecError::InvalidState);
        }
        self.rent.validate()?;
        if self.candidate_kind == SettlementCandidateKindV1::Direct
            && self.settlement_candidate_id != self.base_relation_candidate_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        match self.verification_state {
            ClearWorkVerificationStateV1::Pending => match self.phase {
                0 => {
                    if self.order_cursor != 0
                        || self.slice_cursor != 0
                        || self.page_count != 0
                        || self.page_cursor != 0
                        || self.slot_cursor != 0
                        || self.sha256
                            != (Sha256CheckpointV1 {
                                state: SHA256_INITIAL_STATE_V1,
                                block: [0; 64],
                                block_len: 0,
                                total_len: 0,
                            })
                    {
                        return Err(CodecError::InvalidState);
                    }
                }
                1 => {
                    if self.page_count == 0 || self.page_cursor >= self.page_count {
                        return Err(CodecError::InvalidState);
                    }
                }
                _ => return Err(CodecError::InvalidState),
            },
            ClearWorkVerificationStateV1::Valid => {
                if !matches!(self.phase, 2 | 3)
                    || (self.order_count != 0 && self.page_count == 0)
                    || self.page_cursor != self.page_count
                    || self.slot_cursor != 0
                    || self.order_cursor != self.order_count
                    || (self.phase == 3 && self.slice_cursor != self.slice_count)
                {
                    return Err(CodecError::InvalidState);
                }
            }
            ClearWorkVerificationStateV1::Refused => {
                if self.phase != 3
                    || (self.order_cursor < self.order_count && self.slice_cursor != 0)
                {
                    return Err(CodecError::InvalidState);
                }
            }
        }
        self.sha256.validate()
    }

    /// Encode the exact V3 header. The caller owns the active-width tail.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, CLEAR_WORK_V3_HEADER_BYTES)?;
        header(
            &mut w,
            CLEAR_WORK_ACCOUNT_TAG,
            CLEAR_WORK_ACCOUNT_VERSION_V3,
        )?;
        for id in [
            self.epoch,
            self.node,
            self.market,
            self.order_set,
            self.feed,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.relation_policy_id,
            self.economic_domain_digest,
            self.native_claim_basis_id,
            self.candidate_price_digest,
            self.price_measure_policy_v1_id,
            self.score_policy_id,
            self.price_body_digest,
            self.previous_order_id,
        ] {
            w.bytes(&id.bytes())?;
        }
        for value in [
            self.epoch_generation,
            self.reward_remaining,
            self.reward_earned,
        ] {
            w.u64(value)?;
        }
        write_rent(&mut w, self.rent)?;
        for value in [
            self.slice_count,
            self.slice_cursor,
            self.page_count,
            self.page_cursor,
        ] {
            w.u16(value)?;
        }
        for value in [
            self.outcome_count,
            self.order_count,
            self.order_cursor,
            self.slot_cursor,
            self.phase,
            self.candidate_kind.to_byte(),
            self.price_witness_schema,
            self.quantized_semantics_version,
            self.stored_bump,
            self.verification_state.to_byte(),
            self.flags,
        ] {
            w.u8(value)?;
        }
        write_sha(&mut w, self.sha256)?;
        w.finish()
    }

    /// Decode and validate one exact active-width V3 account.
    pub fn decode_account(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() < CLEAR_WORK_V3_HEADER_BYTES {
            return Err(CodecError::WrongLength);
        }
        let mut r = Reader::exact(
            &input[..CLEAR_WORK_V3_HEADER_BYTES],
            CLEAR_WORK_V3_HEADER_BYTES,
        )?;
        check_header(
            &mut r,
            CLEAR_WORK_ACCOUNT_TAG,
            CLEAR_WORK_ACCOUNT_VERSION_V3,
        )?;
        let value = Self {
            epoch: read_id(&mut r)?,
            node: read_id(&mut r)?,
            market: read_id(&mut r)?,
            order_set: read_id(&mut r)?,
            feed: read_id(&mut r)?,
            candidate_bundle_digest: read_id(&mut r)?,
            settlement_candidate_id: read_id(&mut r)?,
            base_relation_candidate_id: read_id(&mut r)?,
            relation_policy_id: read_id(&mut r)?,
            economic_domain_digest: read_id(&mut r)?,
            native_claim_basis_id: read_id(&mut r)?,
            candidate_price_digest: read_id(&mut r)?,
            price_measure_policy_v1_id: read_id(&mut r)?,
            score_policy_id: read_id(&mut r)?,
            price_body_digest: read_id(&mut r)?,
            previous_order_id: Id32::from_bytes(r.array()?),
            epoch_generation: r.u64()?,
            reward_remaining: r.u64()?,
            reward_earned: r.u64()?,
            rent: read_rent(&mut r)?,
            slice_count: r.u16()?,
            slice_cursor: r.u16()?,
            page_count: r.u16()?,
            page_cursor: r.u16()?,
            outcome_count: r.u8()?,
            order_count: r.u8()?,
            order_cursor: r.u8()?,
            slot_cursor: r.u8()?,
            phase: r.u8()?,
            candidate_kind: SettlementCandidateKindV1::from_byte(r.u8()?)?,
            price_witness_schema: r.u8()?,
            quantized_semantics_version: r.u8()?,
            stored_bump: r.u8()?,
            verification_state: ClearWorkVerificationStateV1::from_byte(r.u8()?)?,
            flags: r.u8()?,
            sha256: read_sha(&mut r)?,
        };
        r.finish()?;
        value.validate()?;
        if input.len() != clear_work_v3_account_len(value.outcome_count, value.order_count)? {
            return Err(CodecError::WrongLength);
        }
        validate_clear_work_v3_tail(input, value)?;
        Ok(value)
    }
}

/// Compute the exact V3 active-width Work account length.
pub fn clear_work_v3_account_len(outcomes: u8, orders: u8) -> Result<usize, CodecError> {
    if !(2..=MAX_OUTCOMES_U8).contains(&outcomes) || orders > MAX_ORDERS_U8 {
        return Err(CodecError::InvalidCount);
    }
    CLEAR_WORK_V3_HEADER_BYTES
        .checked_add(usize::from(outcomes) * 16)
        .and_then(|value| value.checked_add(usize::from(outcomes) * usize::from(orders) * 8))
        .ok_or(CodecError::ArithmeticOverflow)
}

/// Exact aggregate RelationV2 flow vectors persisted in a V3 Work tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkRelationFlowsV1 {
    /// Aggregate filled buy legs; inactive outcomes are canonical zeroes.
    pub aggregate_buy_flow: [u64; MAX_OUTCOMES],
    /// Aggregate filled sell legs; inactive outcomes are canonical zeroes.
    pub aggregate_sell_flow: [u64; MAX_OUTCOMES],
}

/// Decode the canonical aggregate RelationV2 flow vectors from a V3 Work.
pub fn decode_clear_work_v3_relation_flows(
    input: &[u8],
) -> Result<ClearWorkRelationFlowsV1, CodecError> {
    let header = ClearWorkV3AccountV1::decode_account(input)?;
    let mut aggregate_buy_flow = [0u64; MAX_OUTCOMES];
    let mut aggregate_sell_flow = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(header.outcome_count) {
        let at = CLEAR_WORK_V3_HEADER_BYTES
            .checked_add(
                outcome
                    .checked_mul(16)
                    .ok_or(CodecError::ArithmeticOverflow)?,
            )
            .ok_or(CodecError::ArithmeticOverflow)?;
        aggregate_buy_flow[outcome] = read_u64_at(input, at)?;
        aggregate_sell_flow[outcome] = read_u64_at(input, at + 8)?;
        outcome += 1;
    }
    Ok(ClearWorkRelationFlowsV1 {
        aggregate_buy_flow,
        aggregate_sell_flow,
    })
}

/// Decode one already-folded order's exact filled-leg vector.
///
/// Rows at or beyond `order_cursor` remain canonical zeroes and are not
/// observable as accepted relation state.
pub fn decode_clear_work_v3_filled_legs(
    input: &[u8],
    order_index: u8,
) -> Result<[u64; MAX_OUTCOMES], CodecError> {
    let header = ClearWorkV3AccountV1::decode_account(input)?;
    if order_index >= header.order_cursor {
        return Err(CodecError::InvalidState);
    }
    let matrix_at = CLEAR_WORK_V3_HEADER_BYTES
        .checked_add(
            usize::from(header.outcome_count)
                .checked_mul(16)
                .ok_or(CodecError::ArithmeticOverflow)?,
        )
        .ok_or(CodecError::ArithmeticOverflow)?;
    let row_at = matrix_at
        .checked_add(
            usize::from(order_index)
                .checked_mul(usize::from(header.outcome_count))
                .and_then(|value| value.checked_mul(8))
                .ok_or(CodecError::ArithmeticOverflow)?,
        )
        .ok_or(CodecError::ArithmeticOverflow)?;
    let mut filled_legs = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(header.outcome_count) {
        filled_legs[outcome] = read_u64_at(input, row_at + (outcome * 8))?;
        outcome += 1;
    }
    Ok(filled_legs)
}

/// Atomically encode one private runtime-owned RelationV2 order result into
/// the existing V3 Work frame.
///
/// Accepted steps append exactly the row at the prestate dense cursor and
/// replace both aggregate flow vectors. Checked refusal changes only the
/// header disposition/reward facts, leaving every tail byte unchanged.
#[allow(clippy::too_many_arguments)]
pub fn replace_clear_work_v3_order_state(
    account: &mut [u8],
    pre: ClearWorkV3AccountV1,
    post: ClearWorkV3AccountV1,
    aggregate_buy_flow: [u64; MAX_OUTCOMES],
    aggregate_sell_flow: [u64; MAX_OUTCOMES],
    filled_legs: [u64; MAX_OUTCOMES],
    accepted_order: bool,
) -> Result<(), CodecError> {
    if ClearWorkV3AccountV1::decode_account(account)? != pre {
        return Err(CodecError::MismatchedBinding);
    }
    post.validate()?;
    if post.epoch != pre.epoch
        || post.node != pre.node
        || post.market != pre.market
        || post.order_set != pre.order_set
        || post.feed != pre.feed
        || post.candidate_bundle_digest != pre.candidate_bundle_digest
        || post.settlement_candidate_id != pre.settlement_candidate_id
        || post.base_relation_candidate_id != pre.base_relation_candidate_id
        || post.relation_policy_id != pre.relation_policy_id
        || post.economic_domain_digest != pre.economic_domain_digest
        || post.native_claim_basis_id != pre.native_claim_basis_id
        || post.candidate_price_digest != pre.candidate_price_digest
        || post.price_measure_policy_v1_id != pre.price_measure_policy_v1_id
        || post.score_policy_id != pre.score_policy_id
        || post.price_body_digest != pre.price_body_digest
        || post.epoch_generation != pre.epoch_generation
        || post.rent != pre.rent
        || post.slice_count != pre.slice_count
        || post.outcome_count != pre.outcome_count
        || post.order_count != pre.order_count
        || post.candidate_kind != pre.candidate_kind
        || post.price_witness_schema != pre.price_witness_schema
        || post.quantized_semantics_version != pre.quantized_semantics_version
        || post.stored_bump != pre.stored_bump
        || post.flags != pre.flags
        || pre.verification_state != ClearWorkVerificationStateV1::Pending
    {
        return Err(CodecError::MismatchedBinding);
    }
    let expected_cursor = pre
        .order_cursor
        .checked_add(u8::from(accepted_order))
        .ok_or(CodecError::ArithmeticOverflow)?;
    if accepted_order {
        if pre.order_cursor >= pre.order_count
            || post.order_cursor != expected_cursor
            || post.previous_order_id.is_zero()
        {
            return Err(CodecError::InvalidState);
        }
    } else if post.order_cursor != pre.order_cursor
        || post.previous_order_id != pre.previous_order_id
        || post.verification_state != ClearWorkVerificationStateV1::Refused
        || filled_legs.iter().any(|value| *value != 0)
    {
        return Err(CodecError::InvalidState);
    }
    let active = usize::from(pre.outcome_count);
    if aggregate_buy_flow[active..]
        .iter()
        .chain(aggregate_sell_flow[active..].iter())
        .chain(filled_legs[active..].iter())
        .any(|value| *value != 0)
    {
        return Err(CodecError::NonCanonicalPadding);
    }
    let prior_flows = decode_clear_work_v3_relation_flows(account)?;
    if !accepted_order
        && (prior_flows.aggregate_buy_flow != aggregate_buy_flow
            || prior_flows.aggregate_sell_flow != aggregate_sell_flow)
    {
        return Err(CodecError::MismatchedBinding);
    }
    let matrix_at = CLEAR_WORK_V3_HEADER_BYTES
        .checked_add(
            active
                .checked_mul(16)
                .ok_or(CodecError::ArithmeticOverflow)?,
        )
        .ok_or(CodecError::ArithmeticOverflow)?;
    let row_at = matrix_at
        .checked_add(
            usize::from(pre.order_cursor)
                .checked_mul(active)
                .and_then(|value| value.checked_mul(8))
                .ok_or(CodecError::ArithmeticOverflow)?,
        )
        .ok_or(CodecError::ArithmeticOverflow)?;
    let row_end = row_at
        .checked_add(
            active
                .checked_mul(8)
                .ok_or(CodecError::ArithmeticOverflow)?,
        )
        .ok_or(CodecError::ArithmeticOverflow)?;
    if row_end > account.len()
        || (accepted_order && account[row_at..row_end].iter().any(|byte| *byte != 0))
    {
        return Err(CodecError::NonCanonicalPadding);
    }
    let mut encoded_header = [0u8; CLEAR_WORK_V3_HEADER_BYTES];
    post.encode(&mut encoded_header)?;
    if accepted_order {
        let mut outcome = 0usize;
        while outcome < active {
            let flow_at = CLEAR_WORK_V3_HEADER_BYTES + (outcome * 16);
            account[flow_at..flow_at + 8]
                .copy_from_slice(&aggregate_buy_flow[outcome].to_le_bytes());
            account[flow_at + 8..flow_at + 16]
                .copy_from_slice(&aggregate_sell_flow[outcome].to_le_bytes());
            let leg_at = row_at + (outcome * 8);
            account[leg_at..leg_at + 8].copy_from_slice(&filled_legs[outcome].to_le_bytes());
            outcome += 1;
        }
    }
    account[..CLEAR_WORK_V3_HEADER_BYTES].copy_from_slice(&encoded_header);
    Ok(())
}

fn validate_clear_work_v3_tail(
    input: &[u8],
    header: ClearWorkV3AccountV1,
) -> Result<(), CodecError> {
    let accumulators_bytes = usize::from(header.outcome_count)
        .checked_mul(16)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let matrix_at = CLEAR_WORK_V3_HEADER_BYTES
        .checked_add(accumulators_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if header.phase == 0
        && input[CLEAR_WORK_V3_HEADER_BYTES..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(CodecError::NonCanonicalPadding);
    }
    if header.order_cursor < header.order_count {
        let unprocessed_at = matrix_at
            .checked_add(
                usize::from(header.order_cursor)
                    .checked_mul(usize::from(header.outcome_count))
                    .and_then(|value| value.checked_mul(8))
                    .ok_or(CodecError::ArithmeticOverflow)?,
            )
            .ok_or(CodecError::ArithmeticOverflow)?;
        if input[unprocessed_at..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
    }
    Ok(())
}

/// Root Budget with disjoint rent and present-funded reward compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBudgetV2AccountV1 {
    /// Parent Epoch.
    pub epoch: Id32,
    /// Actual General V2 MarketRuntime PDA.
    pub market: Id32,
    /// Immutable liveness/admission policy.
    pub admission_policy_id: Id32,
    /// Payer that capitalized root rewards and selected-artifact rent.
    pub funding_payer: Id32,
    /// Parent generation.
    pub epoch_generation: u64,
    /// Initial/remaining freeze reward.
    pub freeze_initial: u64,
    /// Remaining freeze reward.
    pub freeze_remaining: u64,
    /// Initial finalize reward.
    pub finalize_initial: u64,
    /// Remaining finalize reward.
    pub finalize_remaining: u64,
    /// Initial unique solver prize.
    pub solver_initial: u64,
    /// Remaining solver prize.
    pub solver_remaining: u64,
    /// Initial root-close reward.
    pub root_close_initial: u64,
    /// Remaining root-close reward.
    pub root_close_remaining: u64,
    /// Initial full selected-artifact rent principal.
    pub selected_rent_initial: u64,
    /// Remaining selected-artifact rent principal.
    pub selected_rent_remaining: u64,
    /// Disjoint Budget rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Freeze paid marker.
    pub freeze_paid: u8,
    /// Finalize paid marker.
    pub finalize_paid: u8,
    /// Solver claim state: open/paid/neutralized.
    pub solver_state: u8,
    /// Selected rent state: open/materialized/reclaimed-unused.
    pub selected_rent_state: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags.
    pub flags: u8,
}

/// Semantic-owner proof that an Epoch Budget has no unpaid obligation except
/// the root-close reward consumed by the same atomic retirement transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBudgetRetirementDispositionV1 {
    market: Id32,
    epoch_account: Id32,
    epoch_generation: u64,
    funding_payer: Id32,
    root_close_reward: u64,
    rent: DeletableRentOwnerV1,
}

impl EpochBudgetRetirementDispositionV1 {
    /// Parent Market authenticated by the Budget codec.
    pub const fn market(self) -> Id32 {
        self.market
    }

    /// Parent General V2 Epoch account authenticated by the Budget codec.
    pub const fn epoch_account(self) -> Id32 {
        self.epoch_account
    }

    /// Exact parent generation authenticated by the Budget codec.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }

    /// Original root-funding payer.
    pub const fn funding_payer(self) -> Id32 {
        self.funding_payer
    }

    /// Permissionless root-close reward that must be paid before deletion.
    pub const fn root_close_reward(self) -> u64 {
        self.root_close_reward
    }

    /// Independently owned Budget rent principal and donation floor.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

impl EpochBudgetV2AccountV1 {
    /// Validate exact reward and rent state.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.market,
            self.admission_policy_id,
            self.funding_payer,
        ] {
            live(id)?;
        }
        self.rent.validate()?;
        if self.epoch_generation == 0
            || self.freeze_remaining > self.freeze_initial
            || self.finalize_remaining > self.finalize_initial
            || self.solver_remaining > self.solver_initial
            || self.root_close_initial == 0
            // A successful root close deletes this Budget atomically, so no
            // live serialization may show this liveness compartment spent.
            || self.root_close_remaining != self.root_close_initial
            || self.selected_rent_initial == 0
            || self.selected_rent_remaining > self.selected_rent_initial
            || self.freeze_paid > 1
            || self.finalize_paid > 1
            || self.solver_state > 2
            || self.selected_rent_state > 2
            || (self.freeze_paid == 0 && self.freeze_remaining != self.freeze_initial)
            || (self.freeze_paid == 1 && self.freeze_remaining != 0)
            || (self.finalize_paid == 0 && self.finalize_remaining != self.finalize_initial)
            || (self.finalize_paid == 1 && self.finalize_remaining != 0)
            || (self.solver_state == 0 && self.solver_remaining != self.solver_initial)
            || (self.solver_state != 0 && self.solver_remaining != 0)
            || (self.selected_rent_state == 0
                && self.selected_rent_remaining != self.selected_rent_initial)
            || (self.selected_rent_state != 0 && self.selected_rent_remaining != 0)
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        self.freeze_initial
            .checked_add(self.finalize_initial)
            .and_then(|value| value.checked_add(self.solver_initial))
            .and_then(|value| value.checked_add(self.root_close_initial))
            .and_then(|value| value.checked_add(self.selected_rent_initial))
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Consume the semantic Budget terminality check for atomic root close.
    ///
    /// Freeze/finalize rewards must be paid, solver liability must be paid or
    /// explicitly neutralized, and selected-artifact rent must be materialized
    /// or reclaimed. The root-close reward deliberately remains present: the
    /// root-close transaction pays it once and deletes the Budget atomically.
    pub fn retirement_disposition(self) -> Result<EpochBudgetRetirementDispositionV1, CodecError> {
        self.validate()?;
        if self.freeze_paid != 1
            || self.freeze_remaining != 0
            || self.finalize_paid != 1
            || self.finalize_remaining != 0
            || !(1..=2).contains(&self.solver_state)
            || self.solver_remaining != 0
            || !(1..=2).contains(&self.selected_rent_state)
            || self.selected_rent_remaining != 0
            || self.root_close_remaining != self.root_close_initial
        {
            return Err(CodecError::InvalidState);
        }
        Ok(EpochBudgetRetirementDispositionV1 {
            market: self.market,
            epoch_account: self.epoch,
            epoch_generation: self.epoch_generation,
            funding_payer: self.funding_payer,
            root_close_reward: self.root_close_remaining,
            rent: self.rent,
        })
    }

    /// Encode exactly [`EPOCH_BUDGET_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut w = Writer::exact(out, EPOCH_BUDGET_ACCOUNT_BYTES)?;
        header(
            &mut w,
            EPOCH_BUDGET_ACCOUNT_TAG,
            EPOCH_BUDGET_ACCOUNT_VERSION,
        )?;
        for id in [
            self.epoch,
            self.market,
            self.admission_policy_id,
            self.funding_payer,
        ] {
            w.bytes(&id.bytes())?;
        }
        for value in [
            self.epoch_generation,
            self.freeze_initial,
            self.freeze_remaining,
            self.finalize_initial,
            self.finalize_remaining,
            self.solver_initial,
            self.solver_remaining,
            self.root_close_initial,
            self.root_close_remaining,
            self.selected_rent_initial,
            self.selected_rent_remaining,
        ] {
            w.u64(value)?;
        }
        write_rent(&mut w, self.rent)?;
        for value in [
            self.freeze_paid,
            self.finalize_paid,
            self.solver_state,
            self.selected_rent_state,
            self.stored_bump,
            self.flags,
        ] {
            w.u8(value)?;
        }
        w.finish()
    }

    /// Decode and validate exactly [`EPOCH_BUDGET_ACCOUNT_BYTES`] hostile
    /// bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, EPOCH_BUDGET_ACCOUNT_BYTES)?;
        check_header(
            &mut r,
            EPOCH_BUDGET_ACCOUNT_TAG,
            EPOCH_BUDGET_ACCOUNT_VERSION,
        )?;
        let value = Self {
            epoch: read_id(&mut r)?,
            market: read_id(&mut r)?,
            admission_policy_id: read_id(&mut r)?,
            funding_payer: read_id(&mut r)?,
            epoch_generation: r.u64()?,
            freeze_initial: r.u64()?,
            freeze_remaining: r.u64()?,
            finalize_initial: r.u64()?,
            finalize_remaining: r.u64()?,
            solver_initial: r.u64()?,
            solver_remaining: r.u64()?,
            root_close_initial: r.u64()?,
            root_close_remaining: r.u64()?,
            selected_rent_initial: r.u64()?,
            selected_rent_remaining: r.u64()?,
            rent: read_rent(&mut r)?,
            freeze_paid: r.u8()?,
            finalize_paid: r.u8()?,
            solver_state: r.u8()?,
            selected_rent_state: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Permissionless terminal-node cleanup classification.
///
/// The protected variant is an explicit refusal: a handler must not debit,
/// close, unlink, or transfer any input account when it is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateNodeCleanupDispositionV1 {
    /// The node is the Window-owned working best before finalization.
    ProtectedWorkingBest,
    /// Close the source node while the counted SelectedCandidate retains Feed.
    CloseNodeRetainingSelectedFeed,
    /// Close the terminal node and its authenticated dependent Feed atomically.
    CloseNodeAndFeed,
    /// Close only the terminal node after authenticating canonical Feed absence.
    CloseNodeAfterFeedAbsent,
}

/// Exact paired counter/head transition returned by cleanup classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateNodeCleanupDecisionV1 {
    /// Authorized close kind, or an explicit protected refusal.
    pub disposition: CandidateNodeCleanupDispositionV1,
    /// Epoch candidate-bundle count after the decision is applied.
    pub epoch_candidate_bundle_count_after: u32,
    /// Window live-node count after the decision is applied.
    pub window_live_node_count_after: u64,
    /// Window closed-node count after the decision is applied.
    pub window_closed_node_count_after: u64,
    /// Window reverse-list head after the decision is applied.
    pub window_head_after: Id32,
}

/// Authenticated live SelectedCandidate view used by node cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedCandidateV1<'a> {
    /// Supplied selected-artifact account identity.
    pub artifact: Id32,
    /// Decoded selected-artifact state.
    pub account: &'a SelectedCandidateV1AccountV1,
}

/// Complete read-only state partition required before terminal node cleanup.
///
/// `authenticated_feed == Id32::ZERO` and `authenticated_work == Id32::ZERO`
/// mean the adapter authenticated canonical PDA absence. A nonzero Feed must
/// equal `derived_feed`; a Work must always be absent before cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateNodeCleanupViewV1<'a> {
    /// Decoded terminal AdmissionNode at the reverse-list head.
    pub node: &'a AdmissionNodeV3AccountV1,
    /// Supplied Window account identity.
    pub window_identity: Id32,
    /// Decoded authoritative Window.
    pub window: &'a CandidateWindowV4AccountV1,
    /// Independently decoded successor Epoch generation.
    pub epoch_generation: u64,
    /// Successor Epoch's authoritative candidate-bundle count.
    pub epoch_candidate_bundle_count: u32,
    /// Successor Epoch's unique selected-artifact count.
    pub epoch_selected_candidate_count: u32,
    /// Current authenticated clock slot.
    pub current_slot: u64,
    /// Canonical Feed PDA derived from the node.
    pub derived_feed: Id32,
    /// Canonical predecessor PDA derived from ordinal minus one, or zero.
    pub derived_previous_node: Id32,
    /// Authenticated Feed identity, or zero for canonical absence.
    pub authenticated_feed: Id32,
    /// Authenticated Work identity, or zero for canonical absence.
    pub authenticated_work: Id32,
    /// Live selected artifact when and only when the Epoch count is one.
    pub selected: Option<AuthenticatedSelectedCandidateV1<'a>>,
}

impl CandidateNodeCleanupViewV1<'_> {
    /// Validate the exhaustive Window/node/Feed/artifact partition and classify
    /// the only authorized cleanup transition and paired post-counts.
    pub fn classify(self) -> Result<CandidateNodeCleanupDecisionV1, CodecError> {
        self.node.validate()?;
        self.window.validate()?;
        live(self.window_identity)?;
        live(self.derived_feed)?;
        if self.epoch_generation == 0
            || self.epoch_generation != self.node.epoch_generation
            || self.epoch_generation != self.window.epoch_generation
            || self.epoch_candidate_bundle_count == 0
            || u64::from(self.epoch_candidate_bundle_count) != self.window.live_node_count
            || self.node.ordinal != self.window.live_node_count
            || self.current_slot < self.window.submission_closes_slot
            || self.current_slot < self.node.terminal_slot
            || (self.window.finalized_slot != 0 && self.current_slot < self.window.finalized_slot)
            || !self.authenticated_work.is_zero()
            || self.node.node != self.window.admission_head
            || self.node.epoch != self.window.epoch
            || self.node.market != self.window.market
            || self.node.relation_policy_id != self.window.relation_policy_id
            || self.node.admission_policy_id != self.window.admission_policy_id
            || self.node.score_policy_id != self.window.score_policy_id
            || self.node.epoch_generation != self.window.epoch_generation
            || self.node.node == self.window_identity
            || self.node.node == self.derived_feed
            || self.window_identity == self.derived_feed
            || matches!(
                self.node.status,
                AdmissionNodeStatusV1::Committed | AdmissionNodeStatusV1::Revealed
            )
            || self.epoch_selected_candidate_count > 1
        {
            return Err(CodecError::MismatchedBinding);
        }
        let (count_after, live_after, head_after) = if self.window.live_node_count == 1 {
            if !self.node.previous_node.is_zero() || !self.derived_previous_node.is_zero() {
                return Err(CodecError::MismatchedBinding);
            }
            (0, 0, Id32::ZERO)
        } else {
            live(self.node.previous_node)?;
            if self.derived_previous_node != self.node.previous_node {
                return Err(CodecError::MismatchedBinding);
            }
            (
                self.epoch_candidate_bundle_count
                    .checked_sub(1)
                    .ok_or(CodecError::ArithmeticOverflow)?,
                self.window
                    .live_node_count
                    .checked_sub(1)
                    .ok_or(CodecError::ArithmeticOverflow)?,
                self.node.previous_node,
            )
        };
        let closed_after = self
            .window
            .closed_node_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        let close_decision = |disposition| CandidateNodeCleanupDecisionV1 {
            disposition,
            epoch_candidate_bundle_count_after: count_after,
            window_live_node_count_after: live_after,
            window_closed_node_count_after: closed_after,
            window_head_after: head_after,
        };
        let feed_present = if self.authenticated_feed.is_zero() {
            false
        } else if self.authenticated_feed == self.derived_feed {
            true
        } else {
            return Err(CodecError::MismatchedBinding);
        };

        if self.window.finalized_slot == 0 {
            if self.epoch_selected_candidate_count != 0 || self.selected.is_some() {
                return Err(CodecError::MismatchedBinding);
            }
            if self.node.node == self.window.best_candidate_node {
                if !feed_present
                    || self.node.status != AdmissionNodeStatusV1::VerifiedValid
                    || self.node.settlement_candidate_id != self.window.best_settlement_candidate_id
                    || self.node.rank_key != self.window.best_rank_key
                    || self.node.ordinal != self.window.best_ordinal
                {
                    return Err(CodecError::MismatchedBinding);
                }
                return Ok(CandidateNodeCleanupDecisionV1 {
                    disposition: CandidateNodeCleanupDispositionV1::ProtectedWorkingBest,
                    epoch_candidate_bundle_count_after: self.epoch_candidate_bundle_count,
                    window_live_node_count_after: self.window.live_node_count,
                    window_closed_node_count_after: self.window.closed_node_count,
                    window_head_after: self.window.admission_head,
                });
            }
            return Ok(close_decision(if feed_present {
                CandidateNodeCleanupDispositionV1::CloseNodeAndFeed
            } else {
                CandidateNodeCleanupDispositionV1::CloseNodeAfterFeedAbsent
            }));
        }

        if self.window.selected_candidate_artifact.is_zero() {
            if self.epoch_selected_candidate_count != 0 || self.selected.is_some() {
                return Err(CodecError::MismatchedBinding);
            }
        } else if self.epoch_selected_candidate_count == 1 {
            let selected = self.selected.ok_or(CodecError::MismatchedBinding)?;
            selected.account.validate()?;
            if selected.artifact != self.window.selected_candidate_artifact
                || selected.account.window != self.window_identity
                || selected.account.epoch != self.window.epoch
                || selected.account.market != self.window.market
                || selected.account.epoch_generation != self.window.epoch_generation
            {
                return Err(CodecError::MismatchedBinding);
            }
            if selected.account.source_admission_node == self.node.node {
                if selected.account.selected_feed != self.derived_feed
                    || !feed_present
                    || self.node.status != AdmissionNodeStatusV1::VerifiedValid
                    || selected.account.candidate_bundle_digest != self.node.candidate_bundle_digest
                    || selected.account.settlement_candidate_id != self.node.settlement_candidate_id
                    || selected.account.base_relation_candidate_id
                        != self.node.base_relation_candidate_id
                    || selected.account.settlement_witness_digest
                        != self.node.settlement_witness_digest
                    || selected.account.relation_policy_id != self.node.relation_policy_id
                    || selected.account.score_policy_id != self.node.score_policy_id
                    || selected.account.solver_reward_destination
                        != self.node.solver_reward_destination
                    || selected.account.rank_key != self.node.rank_key
                    || selected.account.ordinal != self.node.ordinal
                    || selected.account.candidate_kind != self.node.candidate_kind
                    || selected.account.selected_slot != self.window.finalized_slot
                {
                    return Err(CodecError::MismatchedBinding);
                }
                return Ok(close_decision(
                    CandidateNodeCleanupDispositionV1::CloseNodeRetainingSelectedFeed,
                ));
            }
            if selected.account.selected_feed == self.derived_feed {
                return Err(CodecError::MismatchedBinding);
            }
        } else if self.epoch_selected_candidate_count != 0 || self.selected.is_some() {
            return Err(CodecError::MismatchedBinding);
        }

        Ok(close_decision(if feed_present {
            CandidateNodeCleanupDispositionV1::CloseNodeAndFeed
        } else {
            CandidateNodeCleanupDispositionV1::CloseNodeAfterFeedAbsent
        }))
    }
}

/// Authenticated close contract for the selected artifact and its retained
/// sealed Feed.
///
/// A handler may close the two accounts only atomically with decrementing the
/// successor Epoch's unique selected-artifact count. Counted receipts,
/// reservations, and pots created by entitlement then own every downstream
/// liability; static clients never substitute for those accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedCandidateRetirementContractV1 {
    /// Parent Epoch identity authenticated from the retirement cursor.
    pub epoch: Id32,
    /// Parent Epoch generation copied by all members.
    pub epoch_generation: u64,
    /// Unique live selected-artifact count in the successor Epoch.
    pub epoch_selected_candidate_count: u32,
    /// Supplied selected-artifact account identity.
    pub artifact: Id32,
    /// Parent Epoch stored by the selected artifact.
    pub artifact_epoch: Id32,
    /// Parent Epoch generation stored by the selected artifact.
    pub artifact_epoch_generation: u64,
    /// Historical identity persisted by the Window.
    pub window_selected_artifact: Id32,
    /// Window identity stored by the selected artifact.
    pub artifact_window: Id32,
    /// Canonical Window PDA derived from the parent Epoch.
    pub derived_window: Id32,
    /// Program-owned Window supplying the historical artifact pointer.
    pub authenticated_window: Id32,
    /// Parent Epoch stored by the Window.
    pub window_epoch: Id32,
    /// Parent Epoch generation stored by the Window.
    pub window_epoch_generation: u64,
    /// Feed identity persisted by the selected artifact.
    pub retained_feed: Id32,
    /// Program-owned sealed Feed identity authenticated for atomic close.
    pub authenticated_feed: Id32,
    /// Parent Epoch stored by the sealed Feed.
    pub feed_epoch: Id32,
    /// Parent Epoch generation stored by the sealed Feed.
    pub feed_epoch_generation: u64,
    /// Exact slice count stored by the sealed Feed.
    pub feed_slice_count: u16,
    /// Canonical Budget PDA derived from the parent Epoch.
    pub derived_budget: Id32,
    /// Program-owned Budget identity supplying solver state.
    pub authenticated_budget: Id32,
    /// Parent Epoch stored by the Budget.
    pub budget_epoch: Id32,
    /// Parent Epoch generation stored by the Budget.
    pub budget_epoch_generation: u64,
    /// Original payer stored by the Budget.
    pub budget_funding_payer: Id32,
    /// Rent payer stored by the selected artifact.
    pub artifact_rent_payer: Id32,
    /// Budget selected-rent state; must be materialized.
    pub budget_selected_rent_state: u8,
    /// Budget selected-rent principal remaining after materialization.
    pub budget_selected_rent_remaining: u64,
    /// Initial selected-artifact rent principal stored by the Budget.
    pub budget_selected_rent_initial: u64,
    /// Refundable rent principal stored by the selected artifact.
    pub artifact_rent_refundable_principal: u64,
    /// Selected artifact's exact slice count.
    pub slice_count: u16,
    /// Selected artifact's materialized slice cursor.
    pub next_slice_index: u16,
    /// Selected artifact entitlement state.
    pub entitlement_state: u8,
    /// Authenticated Budget solver state: paid or explicitly neutralized.
    pub budget_solver_state: u8,
}

impl SelectedCandidateRetirementContractV1 {
    /// Whether every slice has been materialized and the counted artifact/Feed
    /// bundle may retire.
    pub fn retirable(self) -> Result<bool, CodecError> {
        if self.epoch_generation == 0
            || self.artifact_epoch_generation != self.epoch_generation
            || self.feed_epoch_generation != self.epoch_generation
            || self.window_epoch_generation != self.epoch_generation
            || self.budget_epoch_generation != self.epoch_generation
            || self.budget_selected_rent_state != 1
            || self.budget_selected_rent_remaining != 0
            || self.budget_selected_rent_initial == 0
            || self.artifact_rent_refundable_principal == 0
            || self.epoch_selected_candidate_count != 1
            || self.slice_count > MAX_SLICES_U16
            || self.next_slice_index > self.slice_count
            || self.entitlement_state > 2
            || (self.slice_count == 0 && self.entitlement_state != 2)
            || !(1..=2).contains(&self.budget_solver_state)
        {
            return Err(CodecError::InvalidState);
        }
        for id in [
            self.epoch,
            self.artifact,
            self.artifact_epoch,
            self.window_selected_artifact,
            self.artifact_window,
            self.derived_window,
            self.authenticated_window,
            self.window_epoch,
            self.retained_feed,
            self.authenticated_feed,
            self.feed_epoch,
            self.derived_budget,
            self.authenticated_budget,
            self.budget_epoch,
            self.budget_funding_payer,
            self.artifact_rent_payer,
        ] {
            live(id)?;
        }
        if self.artifact_epoch != self.epoch
            || self.feed_epoch != self.epoch
            || self.window_epoch != self.epoch
            || self.budget_epoch != self.epoch
            || self.artifact_window != self.authenticated_window
            || self.derived_window != self.authenticated_window
            || self.derived_budget != self.authenticated_budget
            || self.budget_funding_payer != self.artifact_rent_payer
            || self.budget_selected_rent_initial != self.artifact_rent_refundable_principal
            || self.artifact != self.window_selected_artifact
            || self.retained_feed != self.authenticated_feed
            || self.feed_slice_count != self.slice_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(self.entitlement_state == 2 && self.next_slice_index == self.slice_count)
    }
}

/// Candidate-bundle retirement contract.
///
/// Epoch `candidate_bundles` counts AdmissionNodes, one increment at commit and
/// one decrement only on reverse-head deletion. A non-selected Feed/stage is a
/// dependent in the same logical node bundle and must be absent at node close.
/// A selected node may instead transfer its sealed Feed lifetime to the unique
/// counted SelectedCandidate authority before the node closes. Window
/// `live_node_count` changes atomically with the Epoch candidate-bundle count.
/// Root retirement requires both candidate and selected counts zero plus a zero
/// Window head; the selected retirement contract separately authenticates
/// atomic closure of its retained Feed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateBundleRetirementContractV1 {
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Epoch authoritative candidate-bundle count.
    pub epoch_candidate_bundle_count: u32,
    /// Epoch authoritative selected-candidate artifact count.
    pub epoch_selected_candidate_count: u32,
    /// Window authoritative live-node count.
    pub window_live_node_count: u64,
    /// Window head identity.
    pub window_head: Id32,
}

impl CandidateBundleRetirementContractV1 {
    /// Whether the candidate admission family is exhaustively retired.
    pub fn retired(self) -> Result<bool, CodecError> {
        if self.epoch_generation == 0 || self.epoch_selected_candidate_count > 1 {
            return Err(CodecError::InvalidState);
        }
        if u64::from(self.epoch_candidate_bundle_count) != self.window_live_node_count {
            return Err(CodecError::MismatchedBinding);
        }
        if self.window_live_node_count == 0 {
            absent(self.window_head)?;
        } else {
            live(self.window_head)?;
        }
        Ok(self.window_live_node_count == 0 && self.epoch_selected_candidate_count == 0)
    }
}

fn live(id: Id32) -> Result<(), CodecError> {
    if id.is_zero() {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}
fn validate_rank_candidate_and_ordinal(
    rank: [u8; SCORE_V2_Q_RANK_CAPACITY],
    settlement_candidate_id: Id32,
    ordinal: u64,
) -> Result<(), CodecError> {
    live(settlement_candidate_id)?;
    let final_id = settlement_candidate_id.bytes();
    let mut index = 0usize;
    while index < ID_BYTES {
        if rank[24 + index] != !final_id[index] {
            return Err(CodecError::MismatchedBinding);
        }
        index += 1;
    }
    let coordinate = FirstAdmittedTieV1 { ordinal }.coordinate()?;
    index = 0;
    while index < ID_BYTES {
        if rank[56 + index] != !coordinate[index] {
            return Err(CodecError::MismatchedBinding);
        }
        index += 1;
    }
    if rank[SCORE_V2_Q_ACTIVE_RANK_BYTES..]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(CodecError::NonCanonicalPadding);
    }
    Ok(())
}
fn absent(id: Id32) -> Result<(), CodecError> {
    if id.is_zero() {
        Ok(())
    } else {
        Err(CodecError::NonCanonicalPadding)
    }
}
fn header(w: &mut Writer<'_>, tag: u8, version: u8) -> Result<(), CodecError> {
    w.u8(tag)?;
    w.u8(version)
}
fn check_header(r: &mut Reader<'_>, tag: u8, version: u8) -> Result<(), CodecError> {
    if r.u8()? != tag {
        return Err(CodecError::WrongTag);
    }
    if r.u8()? != version {
        return Err(CodecError::WrongVersion);
    }
    Ok(())
}
fn read_id(r: &mut Reader<'_>) -> Result<Id32, CodecError> {
    Id32::new(r.array()?)
}
fn write_economic_domain_transcript(
    w: &mut Writer<'_>,
    value: EconomicDomainV2Transcript,
) -> Result<(), CodecError> {
    value.validate()?;
    w.u32(value.relation_version)?;
    for id in [
        value.market_instance_v2_id,
        value.epoch_semantics_digest,
        value.relation_policy_id,
        value.price_measure_policy_v1_id,
        value.native_claim_basis_id,
    ] {
        w.bytes(&id.bytes())?;
    }
    w.u64(value.epoch_index)?;
    w.u8(value.outcome_count)?;
    w.u64(value.price_scale)?;
    w.u128(value.coordinate_domain_min)?;
    w.u128(value.coordinate_domain_max)
}
fn read_economic_domain_transcript(
    r: &mut Reader<'_>,
) -> Result<EconomicDomainV2Transcript, CodecError> {
    let value = EconomicDomainV2Transcript {
        relation_version: r.u32()?,
        market_instance_v2_id: read_id(r)?,
        epoch_semantics_digest: read_id(r)?,
        relation_policy_id: read_id(r)?,
        price_measure_policy_v1_id: read_id(r)?,
        native_claim_basis_id: read_id(r)?,
        epoch_index: r.u64()?,
        outcome_count: r.u8()?,
        price_scale: r.u64()?,
        coordinate_domain_min: r.u128()?,
        coordinate_domain_max: r.u128()?,
    };
    value.validate()?;
    Ok(value)
}
fn write_rent(w: &mut Writer<'_>, rent: DeletableRentOwnerV1) -> Result<(), CodecError> {
    rent.validate()?;
    w.bytes(&rent.payer.bytes())?;
    w.u64(rent.refundable_principal)?;
    w.u64(rent.donation_floor)
}
fn read_rent(r: &mut Reader<'_>) -> Result<DeletableRentOwnerV1, CodecError> {
    let value = DeletableRentOwnerV1 {
        payer: read_id(r)?,
        refundable_principal: r.u64()?,
        donation_floor: r.u64()?,
    };
    value.validate()?;
    Ok(value)
}
fn read_sha(r: &mut Reader<'_>) -> Result<Sha256CheckpointV1, CodecError> {
    let mut state = [0u32; 8];
    let mut i = 0;
    while i < 8 {
        state[i] = r.u32()?;
        i += 1;
    }
    let value = Sha256CheckpointV1 {
        state,
        block: r.array()?,
        block_len: r.u8()?,
        total_len: r.u64()?,
    };
    value.validate()?;
    Ok(value)
}
fn write_sha(w: &mut Writer<'_>, value: Sha256CheckpointV1) -> Result<(), CodecError> {
    value.validate()?;
    for word in value.state {
        w.u32(word)?;
    }
    w.bytes(&value.block)?;
    w.u8(value.block_len)?;
    w.u64(value.total_len)
}

const _: () = assert!(ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES == 213);
const _: () = assert!(EPOCH_SEMANTICS_V1_TRANSCRIPT_BYTES == 56);
const _: () = assert!(MARKET_RUNTIME_ACCOUNT_BYTES == 2 + (2 * 32) + (4 * 8) + 48 + 2);
const _: () = assert!(GENERAL_EPOCH_ACCOUNT_BYTES == 2 + (7 * 32) + (4 * 8) + (3 * 4) + 48 + 3);
const _: () = assert!(QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES == 149);
const _: () = assert!(CANDIDATE_BUNDLE_V1_FIXED_BYTES == 369);
const _: () = assert!(ECONOMIC_DOMAIN_ACCOUNT_BYTES == 297);
const _: () =
    assert!(SELECTED_CANDIDATE_ACCOUNT_BYTES == 2 + (19 * 32) + 96 + (3 * 8) + 4 + 48 + 7);
const _: () = assert!(SHA256_CHECKPOINT_BYTES == 105);
const _: () =
    assert!(WINDOW_ACCOUNT_BYTES == 2 + (5 * 32) + (6 * 8) + (4 * 32) + 96 + (10 * 8) + 48 + 3);
const _: () = assert!(ADMISSION_NODE_ACCOUNT_BYTES == 2 + (16 * 32) + 96 + (10 * 8) + 48 + 5);
const _: () = assert!(CANDIDATE_FEED_HEADER_BYTES == 2 + (13 * 32) + (7 * 8) + 16 + 48);
const _: () = assert!(
    CLEAR_WORK_HEADER_BYTES == 2 + (15 * 32) + (3 * 8) + 48 + 4 + 9 + SHA256_CHECKPOINT_BYTES
);
const _: () = assert!(
    CLEAR_WORK_V3_HEADER_BYTES == 2 + (16 * 32) + (3 * 8) + 48 + 8 + 11 + SHA256_CHECKPOINT_BYTES
);
const _: () = assert!(EPOCH_BUDGET_ACCOUNT_BYTES == 2 + (4 * 32) + (11 * 8) + 48 + 6);
const _: () = assert!(MARKET_BINDING_ACCOUNT_BYTES == 2 + (12 * 32) + (18 * 8) + 4 + 6);
const _: () = assert!(candidate_feed_max_len() == 6970);
const _: () = assert!(clear_work_max_len() == 9120);
const _: () = assert!(clear_work_max_len() < 10 * 1024);
const _: () = assert!(clear_work_v3_max_len() == 9158);
const _: () = assert!(clear_work_v3_max_len() < 10 * 1024);

const fn candidate_feed_max_len() -> usize {
    CANDIDATE_FEED_HEADER_BYTES
        + (MAX_OUTCOMES * 8)
        + (MAX_ORDERS * 8)
        + (MAX_QUANTIZED_ATOMS * QUANTIZED_ATOM_BYTES)
        + (MAX_SLICES * SETTLEMENT_SLICE_BYTES)
}
const fn clear_work_max_len() -> usize {
    CLEAR_WORK_HEADER_BYTES + (MAX_OUTCOMES * 16) + (MAX_OUTCOMES * MAX_ORDERS * 8)
}

const fn clear_work_v3_max_len() -> usize {
    CLEAR_WORK_V3_HEADER_BYTES + (MAX_OUTCOMES * 16) + (MAX_OUTCOMES * MAX_ORDERS * 8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        encode_score_v2_q_first_admitted_tie_v1, FirstAdmittedTieV1, ScoreV2QComponentsV1,
    };
    use sha2::{Digest, Sha256};

    struct LengthSha;

    impl Sha256BackendV1 for LengthSha {
        fn sha256(&self, parts: &[&[u8]]) -> [u8; ID_BYTES] {
            let mut out = [0u8; ID_BYTES];
            let total = parts.iter().fold(0usize, |sum, part| sum + part.len());
            out[0] = u8::try_from(total).unwrap();
            out
        }
    }

    struct Sha2Backend;

    impl Sha256BackendV1 for Sha2Backend {
        fn sha256(&self, parts: &[&[u8]]) -> [u8; ID_BYTES] {
            let mut hash = Sha256::new();
            for part in parts {
                hash.update(part);
            }
            hash.finalize().into()
        }
    }

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn economic_domain(outcome_count: u8, price_scale: u64) -> EconomicDomainV2Transcript {
        EconomicDomainV2Transcript {
            relation_version: 2,
            market_instance_v2_id: id(1),
            epoch_semantics_digest: id(2),
            relation_policy_id: id(3),
            price_measure_policy_v1_id: id(4),
            native_claim_basis_id: id(5),
            epoch_index: 7,
            outcome_count,
            price_scale,
            coordinate_domain_min: 10,
            coordinate_domain_max: 1_000,
        }
    }
    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1 {
            payer: id(250),
            refundable_principal: 10,
            donation_floor: 3,
        }
    }
    fn binding() -> MarketBindingV1 {
        MarketBindingV1 {
            market: id(1),
            market_genesis_profile_v2_id: id(2),
            market_instance_v2_id: id(3),
            series_plan_v5_id: id(4),
            series_funding_terms_v2_id: id(5),
            relation_policy_id: id(6),
            price_measure_policy_v1_id: id(7),
            native_claim_basis_id: id(8),
            admission_policy_id: id(9),
            score_policy_id: id(10),
            settlement_policy_id: id(11),
            neutral_sink: id(12),
            price_scale: 100,
            commit_span_slots: 10,
            reveal_span_slots: 10,
            verification_span_slots: 20,
            bond_lamports: 1000,
            invalidity_penalty: 100,
            abandonment_penalty: 50,
            node_cleanup_reward: 10,
            price_check_reward: 1,
            order_reward: 1,
            slice_reward: 1,
            completion_reward: 1,
            work_close_reward: 1,
            feed_close_reward: 1,
            freeze_reward: 1,
            finalize_reward: 1,
            solver_prize: 1,
            root_close_reward: 1,
            relation_version: 2,
            outcome_count: 3,
            basis_degree: 2,
            rank_key_len: 88,
            candidate_kind_mask: 1,
            stored_bump: 4,
            flags: 0,
        }
    }

    fn empty_window() -> CandidateWindowV4AccountV1 {
        CandidateWindowV4AccountV1 {
            epoch: id(1),
            market: id(2),
            relation_policy_id: id(3),
            admission_policy_id: id(4),
            score_policy_id: id(5),
            freeze_deadline_slot: 10,
            frozen_slot: 0,
            reveal_opens_slot: 0,
            submission_closes_slot: 0,
            verification_closes_slot: 0,
            finalized_slot: 0,
            admission_head: Id32::ZERO,
            best_candidate_node: Id32::ZERO,
            best_settlement_candidate_id: Id32::ZERO,
            selected_candidate_artifact: Id32::ZERO,
            best_rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
            admitted_count: 0,
            revealed_count: 0,
            verdict_count: 0,
            valid_verdict_count: 0,
            expired_commitment_count: 0,
            expired_unverified_count: 0,
            live_node_count: 0,
            closed_node_count: 0,
            best_ordinal: 0,
            epoch_generation: 1,
            rent: rent(),
            rank_key_len: 88,
            stored_bump: 7,
            flags: 0,
        }
    }

    fn committed_node() -> AdmissionNodeV3AccountV1 {
        AdmissionNodeV3AccountV1 {
            epoch: id(1),
            market: id(2),
            relation_policy_id: id(3),
            node: id(4),
            previous_node: Id32::ZERO,
            admission_policy_id: id(5),
            score_policy_id: id(6),
            commitment: id(7),
            submitter_authority: id(8),
            solver_reward_destination: id(9),
            payer: id(10),
            refund_destination: id(11),
            candidate_bundle_digest: Id32::ZERO,
            settlement_candidate_id: Id32::ZERO,
            base_relation_candidate_id: Id32::ZERO,
            settlement_witness_digest: Id32::ZERO,
            rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
            epoch_generation: 1,
            ordinal: 1,
            committed_slot: 20,
            window_frozen_slot: 10,
            revealed_slot: 0,
            terminal_slot: 0,
            rent: rent(),
            bond_lamports: 1000,
            cleanup_reward: 5,
            work_escrow_lamports: 0,
            work_funding_initial: 0,
            rank_key_len: 0,
            candidate_kind: SettlementCandidateKindV1::Direct,
            status: AdmissionNodeStatusV1::Committed,
            stored_bump: 8,
            flags: 0,
        }
    }

    fn verified_node() -> AdmissionNodeV3AccountV1 {
        let mut node = committed_node();
        node.candidate_bundle_digest = id(12);
        node.settlement_candidate_id = id(13);
        node.base_relation_candidate_id = id(13);
        node.settlement_witness_digest = id(14);
        node.revealed_slot = 30;
        node.terminal_slot = 40;
        node.work_funding_initial = 100;
        node.work_escrow_lamports = 0;
        node.rank_key_len = 88;
        node.status = AdmissionNodeStatusV1::VerifiedValid;
        node.rank_key = encode_score_v2_q_first_admitted_tie_v1(
            ScoreV2QComponentsV1 {
                certified_risk_flow_atoms: 7,
                cash_equivalent_direct_flow_atoms: 3,
                virtual_churn_atoms: 2,
                settlement_candidate_id: node.settlement_candidate_id,
            },
            FirstAdmittedTieV1 {
                ordinal: node.ordinal,
            },
        )
        .unwrap();
        node
    }

    fn feed_header() -> CandidateFeedHeaderV2 {
        CandidateFeedHeaderV2 {
            epoch: id(1),
            node: id(2),
            market: id(3),
            order_set: id(4),
            relation_policy_id: id(5),
            economic_domain_digest: id(6),
            native_claim_basis_id: id(7),
            candidate_price_digest: id(8),
            price_measure_policy_v1_id: id(9),
            settlement_candidate_id: id(10),
            base_relation_candidate_id: id(10),
            settlement_witness_digest: id(11),
            price_body_digest: id(12),
            epoch_generation: 1,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            price_scale: 100,
            common_denominator: 1,
            close_reward_lamports: 1,
            basis_degree: 2,
            outcome_count: 3,
            order_count: 1,
            atom_count: 1,
            slice_count: 0,
            prices_written: 3,
            fills_written: 1,
            atoms_written: 1,
            slices_written: 0,
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            rent: rent(),
            stored_bump: 9,
            flags: 0,
        }
    }

    fn work_header() -> ClearWorkHeaderV2 {
        ClearWorkHeaderV2 {
            epoch: id(1),
            node: id(2),
            market: id(3),
            order_set: id(4),
            feed: id(5),
            candidate_bundle_digest: id(6),
            settlement_candidate_id: id(7),
            base_relation_candidate_id: id(7),
            relation_policy_id: id(8),
            economic_domain_digest: id(9),
            native_claim_basis_id: id(10),
            candidate_price_digest: id(11),
            price_measure_policy_v1_id: id(12),
            score_policy_id: id(13),
            price_body_digest: id(14),
            epoch_generation: 1,
            rent: rent(),
            reward_remaining: 20,
            reward_earned: 0,
            slice_count: 0,
            slice_cursor: 0,
            outcome_count: 3,
            order_count: 1,
            order_cursor: 0,
            phase: 0,
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            stored_bump: 10,
            flags: 0,
            sha256: Sha256CheckpointV1 {
                state: SHA256_INITIAL_STATE_V1,
                block: [0; 64],
                block_len: 0,
                total_len: 0,
            },
        }
    }

    fn work_v3_header() -> ClearWorkV3AccountV1 {
        let work = work_header();
        ClearWorkV3AccountV1 {
            epoch: work.epoch,
            node: work.node,
            market: work.market,
            order_set: work.order_set,
            feed: work.feed,
            candidate_bundle_digest: work.candidate_bundle_digest,
            settlement_candidate_id: work.settlement_candidate_id,
            base_relation_candidate_id: work.base_relation_candidate_id,
            relation_policy_id: work.relation_policy_id,
            economic_domain_digest: work.economic_domain_digest,
            native_claim_basis_id: work.native_claim_basis_id,
            candidate_price_digest: work.candidate_price_digest,
            price_measure_policy_v1_id: work.price_measure_policy_v1_id,
            score_policy_id: work.score_policy_id,
            price_body_digest: work.price_body_digest,
            previous_order_id: Id32::ZERO,
            epoch_generation: work.epoch_generation,
            rent: work.rent,
            reward_remaining: work.reward_remaining,
            reward_earned: work.reward_earned,
            slice_count: work.slice_count,
            slice_cursor: 0,
            page_count: 0,
            page_cursor: 0,
            outcome_count: work.outcome_count,
            order_count: work.order_count,
            order_cursor: 0,
            slot_cursor: 0,
            phase: 0,
            candidate_kind: work.candidate_kind,
            price_witness_schema: work.price_witness_schema,
            quantized_semantics_version: work.quantized_semantics_version,
            stored_bump: 11,
            verification_state: ClearWorkVerificationStateV1::Pending,
            flags: 0,
            sha256: work.sha256,
        }
    }

    fn budget() -> EpochBudgetV2AccountV1 {
        EpochBudgetV2AccountV1 {
            epoch: id(1),
            market: id(2),
            admission_policy_id: id(3),
            funding_payer: id(4),
            epoch_generation: 1,
            freeze_initial: 10,
            freeze_remaining: 10,
            finalize_initial: 11,
            finalize_remaining: 11,
            solver_initial: 12,
            solver_remaining: 12,
            root_close_initial: 13,
            root_close_remaining: 13,
            selected_rent_initial: 14,
            selected_rent_remaining: 14,
            rent: rent(),
            freeze_paid: 0,
            finalize_paid: 0,
            solver_state: 0,
            selected_rent_state: 0,
            stored_bump: 11,
            flags: 0,
        }
    }

    fn selected_candidate() -> SelectedCandidateV1AccountV1 {
        let settlement_candidate_id = id(10);
        SelectedCandidateV1AccountV1 {
            epoch: id(1),
            market: id(2),
            window: id(3),
            market_binding: id(4),
            source_admission_node: id(5),
            selected_feed: id(6),
            order_set: id(7),
            economic_domain_digest: id(8),
            candidate_bundle_digest: id(9),
            settlement_candidate_id,
            base_relation_candidate_id: settlement_candidate_id,
            settlement_witness_digest: id(11),
            relation_policy_id: id(12),
            price_measure_policy_v1_id: id(13),
            native_claim_basis_id: id(14),
            candidate_price_digest: id(15),
            price_body_digest: id(16),
            score_policy_id: id(17),
            solver_reward_destination: id(18),
            rank_key: encode_score_v2_q_first_admitted_tie_v1(
                ScoreV2QComponentsV1 {
                    certified_risk_flow_atoms: 7,
                    cash_equivalent_direct_flow_atoms: 3,
                    virtual_churn_atoms: 2,
                    settlement_candidate_id,
                },
                FirstAdmittedTieV1 { ordinal: 1 },
            )
            .unwrap(),
            epoch_generation: 1,
            ordinal: 1,
            selected_slot: 30,
            slice_count: 1,
            next_slice_index: 0,
            rent: rent(),
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            rank_key_len: 88,
            entitlement_state: 0,
            stored_bump: 12,
            flags: 0,
        }
    }

    fn working_best_window(node: AdmissionNodeV3AccountV1) -> CandidateWindowV4AccountV1 {
        let mut window = empty_window();
        window.epoch = node.epoch;
        window.market = node.market;
        window.relation_policy_id = node.relation_policy_id;
        window.admission_policy_id = node.admission_policy_id;
        window.score_policy_id = node.score_policy_id;
        window.frozen_slot = 10;
        window.reveal_opens_slot = 20;
        window.submission_closes_slot = 35;
        window.verification_closes_slot = 50;
        window.admission_head = node.node;
        window.best_candidate_node = node.node;
        window.best_settlement_candidate_id = node.settlement_candidate_id;
        window.best_rank_key = node.rank_key;
        window.admitted_count = 1;
        window.revealed_count = 1;
        window.verdict_count = 1;
        window.valid_verdict_count = 1;
        window.live_node_count = 1;
        window.best_ordinal = node.ordinal;
        window
    }

    #[test]
    fn exact_lengths_are_pinned_and_stay_below_one_cpi_creation_ceiling() {
        assert_eq!(MARKET_RUNTIME_ACCOUNT_BYTES, 148);
        assert_eq!(GENERAL_EPOCH_ACCOUNT_BYTES, 321);
        assert_eq!(WINDOW_ACCOUNT_BYTES, 565);
        assert_eq!(ADMISSION_NODE_ACCOUNT_BYTES, 743);
        assert_eq!(CANDIDATE_FEED_HEADER_BYTES, 538);
        assert_eq!(candidate_feed_account_len(16, 64, 16, 416), Ok(6970));
        assert_eq!(CLEAR_WORK_HEADER_BYTES, 672);
        assert_eq!(clear_work_account_len(16, 64), Ok(9120));
        assert_eq!(CLEAR_WORK_V3_HEADER_BYTES, 710);
        assert_eq!(clear_work_v3_account_len(16, 64), Ok(9158));
        assert_eq!(EPOCH_BUDGET_ACCOUNT_BYTES, 272);
        assert_eq!(MARKET_BINDING_ACCOUNT_BYTES, 540);
        assert_eq!(ECONOMIC_DOMAIN_ACCOUNT_BYTES, 297);
        assert_eq!(SELECTED_CANDIDATE_ACCOUNT_BYTES, 789);
    }

    #[test]
    fn market_binding_round_trips_and_refuses_hostile_frames() {
        let value = binding();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES];
        value.encode(&mut bytes).unwrap();
        assert_eq!(MarketBindingV1::decode(&bytes), Ok(value));
        assert_eq!(
            MarketBindingV1::decode(&bytes[..bytes.len() - 1]),
            Err(CodecError::WrongLength)
        );
        let mut wrong = bytes;
        wrong[0] ^= 1;
        assert_eq!(MarketBindingV1::decode(&wrong), Err(CodecError::WrongTag));
        let mut wrong = bytes;
        wrong[1] ^= 1;
        assert_eq!(
            MarketBindingV1::decode(&wrong),
            Err(CodecError::WrongVersion)
        );
        let mut wrong = bytes;
        wrong[2..34].fill(0);
        assert_eq!(
            MarketBindingV1::decode(&wrong),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[test]
    fn runtime_and_general_epoch_round_trip_and_refuse_hostile_state() {
        let runtime = MarketRuntimeV3AccountV1 {
            market_binding: id(1),
            market_instance_v2_id: id(2),
            next_epoch_index: 7,
            next_epoch_generation: 8,
            created_epoch_count: 7,
            retired_epoch_count: 2,
            rent: rent(),
            stored_bump: 3,
            flags: 0,
        };
        assert_eq!(runtime.live_epoch_count(), Ok(5));
        let mut runtime_bytes = [0u8; MARKET_RUNTIME_ACCOUNT_BYTES];
        runtime.encode(&mut runtime_bytes).unwrap();
        assert_eq!(
            MarketRuntimeV3AccountV1::decode(&runtime_bytes),
            Ok(runtime)
        );
        assert_eq!(
            MarketRuntimeV3AccountV1::decode(&runtime_bytes[..runtime_bytes.len() - 1]),
            Err(CodecError::WrongLength)
        );
        let mut wrong = runtime_bytes;
        wrong[1] = 2;
        assert_eq!(
            MarketRuntimeV3AccountV1::decode(&wrong),
            Err(CodecError::WrongVersion)
        );
        let mut bad_runtime = runtime;
        bad_runtime.retired_epoch_count = 8;
        assert_eq!(bad_runtime.validate(), Err(CodecError::InvalidState));

        let open = GeneralEpochV6AccountV1 {
            market_binding: runtime.market_binding,
            market_runtime: id(3),
            market_instance_v2_id: runtime.market_instance_v2_id,
            economic_domain: id(4),
            window: id(5),
            budget: id(6),
            order_set: Id32::ZERO,
            epoch_index: 7,
            generation: 8,
            freeze_deadline_slot: 100,
            frozen_slot: 0,
            candidate_bundle_count: 0,
            work_count: 0,
            selected_candidate_count: 0,
            rent: rent(),
            phase: GeneralEpochPhaseV1::Open,
            stored_bump: 4,
            flags: 0,
        };
        let mut epoch_bytes = [0u8; GENERAL_EPOCH_ACCOUNT_BYTES];
        open.encode(&mut epoch_bytes).unwrap();
        assert_eq!(GeneralEpochV6AccountV1::decode(&epoch_bytes), Ok(open));
        epoch_bytes[2 + (6 * ID_BYTES)] = 9;
        assert_eq!(
            GeneralEpochV6AccountV1::decode(&epoch_bytes),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut frozen = open;
        frozen.order_set = id(7);
        frozen.frozen_slot = 100;
        frozen.phase = GeneralEpochPhaseV1::Frozen;
        frozen.candidate_bundle_count = 1;
        frozen.work_count = 2;
        assert_eq!(frozen.validate(), Err(CodecError::InvalidState));
        frozen.work_count = 0;
        frozen.candidate_bundle_count = 0;
        frozen.selected_candidate_count = 1;
        frozen.phase = GeneralEpochPhaseV1::Finalized;
        assert_eq!(
            frozen.validate(),
            Ok(()),
            "selected settlement authority may outlive its source node"
        );
    }

    #[test]
    fn semantic_digests_are_typed_nonzero_and_field_sensitive() {
        let semantics = EpochSemanticsV1 {
            market_instance_v2_id: id(1),
            epoch_index: 7,
            generation: 8,
            freeze_deadline_slot: 100,
        };
        let digest = epoch_semantics_digest_v1(&Sha2Backend, semantics).unwrap();
        assert!(!digest.is_zero());
        assert_ne!(
            digest,
            epoch_semantics_digest_v1(
                &Sha2Backend,
                EpochSemanticsV1 {
                    epoch_index: 8,
                    ..semantics
                }
            )
            .unwrap()
        );
        let economic = id(9);
        let empty_orders = empty_order_set_digest_v1(&Sha2Backend, economic).unwrap();
        assert!(!empty_orders.is_zero());
        assert_ne!(
            empty_orders,
            empty_order_set_digest_v1(&Sha2Backend, id(10)).unwrap()
        );
        struct ZeroSha;
        impl Sha256BackendV1 for ZeroSha {
            fn sha256(&self, _parts: &[&[u8]]) -> [u8; ID_BYTES] {
                [0; ID_BYTES]
            }
        }
        assert_eq!(
            epoch_semantics_digest_v1(&ZeroSha, semantics),
            Err(CodecError::ZeroIdentity),
            "the backend seam may never smuggle an all-zero live identity"
        );

        let opening = CandidateCommitmentOpeningV1 {
            epoch: id(1),
            market: id(2),
            relation_policy_id: id(3),
            admission_policy_id: id(4),
            score_policy_id: id(5),
            frozen_slot: 6,
            submitter_authority: id(7),
            solver_reward_destination: id(8),
            candidate_bundle_digest: id(9),
            secret: [10; 32],
        };
        let commitment = candidate_commitment_v1(&Sha2Backend, opening).unwrap();
        assert_ne!(
            commitment,
            candidate_commitment_v1(
                &Sha2Backend,
                CandidateCommitmentOpeningV1 {
                    secret: [11; 32],
                    ..opening
                }
            )
            .unwrap()
        );
    }

    #[test]
    fn window_node_and_budget_round_trip_exact_frames() {
        let window = empty_window();
        let mut window_bytes = [0u8; WINDOW_ACCOUNT_BYTES];
        window.encode(&mut window_bytes).unwrap();
        assert_eq!(
            CandidateWindowV4AccountV1::decode(&window_bytes),
            Ok(window)
        );
        assert_eq!(
            CandidateWindowV4AccountV1::decode(&window_bytes[..WINDOW_ACCOUNT_BYTES - 1]),
            Err(CodecError::WrongLength)
        );

        for node in [committed_node(), verified_node()] {
            let mut node_bytes = [0u8; ADMISSION_NODE_ACCOUNT_BYTES];
            node.encode(&mut node_bytes).unwrap();
            assert_eq!(AdmissionNodeV3AccountV1::decode(&node_bytes), Ok(node));
        }
        let mut node_bytes = [0u8; ADMISSION_NODE_ACCOUNT_BYTES];
        verified_node().encode(&mut node_bytes).unwrap();
        node_bytes[2 + (16 * ID_BYTES) + 24] ^= 1;
        assert_eq!(
            AdmissionNodeV3AccountV1::decode(&node_bytes),
            Err(CodecError::MismatchedBinding)
        );
        let mut self_refunding_node = verified_node();
        self_refunding_node.rent.payer = self_refunding_node.node;
        assert_eq!(
            self_refunding_node.validate(),
            Err(CodecError::InvalidState)
        );

        let budget = budget();
        let mut budget_bytes = [0u8; EPOCH_BUDGET_ACCOUNT_BYTES];
        budget.encode(&mut budget_bytes).unwrap();
        assert_eq!(EpochBudgetV2AccountV1::decode(&budget_bytes), Ok(budget));
        let terminal_budget = EpochBudgetV2AccountV1 {
            freeze_remaining: 0,
            finalize_remaining: 0,
            solver_remaining: 0,
            selected_rent_remaining: 0,
            freeze_paid: 1,
            finalize_paid: 1,
            solver_state: 1,
            selected_rent_state: 1,
            ..budget
        };
        let terminal = terminal_budget.retirement_disposition().unwrap();
        assert_eq!(terminal.market(), terminal_budget.market);
        assert_eq!(terminal.epoch_account(), terminal_budget.epoch);
        assert_eq!(
            terminal.epoch_generation(),
            terminal_budget.epoch_generation
        );
        assert_eq!(terminal.funding_payer(), terminal_budget.funding_payer);
        assert_eq!(
            terminal.root_close_reward(),
            terminal_budget.root_close_initial
        );
        assert_eq!(terminal.rent(), terminal_budget.rent);
        budget_bytes[EPOCH_BUDGET_ACCOUNT_BYTES - 1] = 1;
        assert_eq!(
            EpochBudgetV2AccountV1::decode(&budget_bytes),
            Err(CodecError::InvalidState)
        );
        let mut depleted_root_close = budget;
        depleted_root_close.root_close_remaining = 0;
        assert_eq!(
            depleted_root_close.validate(),
            Err(CodecError::InvalidState)
        );
        for hostile in [
            EpochBudgetV2AccountV1 {
                freeze_paid: 0,
                freeze_remaining: budget.freeze_initial,
                ..terminal_budget
            },
            EpochBudgetV2AccountV1 {
                finalize_paid: 0,
                finalize_remaining: budget.finalize_initial,
                ..terminal_budget
            },
            EpochBudgetV2AccountV1 {
                solver_state: 0,
                solver_remaining: budget.solver_initial,
                ..terminal_budget
            },
            EpochBudgetV2AccountV1 {
                selected_rent_state: 0,
                selected_rent_remaining: budget.selected_rent_initial,
                ..terminal_budget
            },
        ] {
            assert_eq!(
                hostile.retirement_disposition(),
                Err(CodecError::InvalidState)
            );
        }

        let selected = selected_candidate();
        let mut selected_bytes = [0u8; SELECTED_CANDIDATE_ACCOUNT_BYTES];
        selected.encode(&mut selected_bytes).unwrap();
        assert_eq!(
            SelectedCandidateV1AccountV1::decode(&selected_bytes),
            Ok(selected)
        );
        selected_bytes[2 + (9 * ID_BYTES)] ^= 1;
        assert_eq!(
            SelectedCandidateV1AccountV1::decode(&selected_bytes),
            Err(CodecError::MismatchedBinding)
        );
        let empty_open = SelectedCandidateV1AccountV1 {
            slice_count: 0,
            ..selected
        };
        assert_eq!(empty_open.validate(), Err(CodecError::InvalidState));
        assert_eq!(
            SelectedCandidateV1AccountV1 {
                entitlement_state: 2,
                ..empty_open
            }
            .validate(),
            Ok(())
        );
    }

    #[test]
    fn finalization_requires_exhaustion_and_moves_best_truth_to_the_artifact() {
        let mut unsafe_window = empty_window();
        unsafe_window.frozen_slot = 10;
        unsafe_window.reveal_opens_slot = 20;
        unsafe_window.submission_closes_slot = 30;
        unsafe_window.verification_closes_slot = 40;
        unsafe_window.finalized_slot = 30;
        unsafe_window.admission_head = id(20);
        unsafe_window.admitted_count = 1;
        unsafe_window.live_node_count = 1;
        unsafe_window.selected_candidate_artifact = id(21);
        unsafe_window.valid_verdict_count = 1;
        assert_eq!(unsafe_window.validate(), Err(CodecError::InvalidState));

        let mut safe_at_submission_close = unsafe_window;
        safe_at_submission_close.revealed_count = 1;
        safe_at_submission_close.verdict_count = 1;
        assert_eq!(safe_at_submission_close.validate(), Ok(()));

        let mut duplicate_best_truth = safe_at_submission_close;
        duplicate_best_truth.best_candidate_node = id(22);
        assert_eq!(
            duplicate_best_truth.validate(),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    #[test]
    fn candidate_funding_is_exact_and_overflow_checked() {
        let funding =
            required_candidate_funding_v1(binding(), 2, 3, rent(), rent(), rent()).unwrap();
        assert_eq!(funding.work_reward_reserve, 8);
        assert_eq!(funding.feed_close_reward, 1);
        assert_eq!(funding.node_cleanup_reward, 10);
        assert_eq!(funding.node_allocation, 1_020);
        assert_eq!(funding.feed_allocation, 11);
        assert_eq!(funding.work_allocation, 18);
        assert_eq!(funding.commit_payer_funding, 1_020);
        assert_eq!(funding.reveal_payer_funding, 29);
        assert_eq!(funding.lifetime_payer_funding, 1_049);
        assert_eq!(funding.node_balance_with_prefund, 1_023);
        assert_eq!(funding.feed_balance_with_prefund, 14);
        assert_eq!(funding.work_balance_with_prefund, 21);
        assert_eq!(funding.lifetime_balance_with_prefunds, 1_058);

        let mut overflow = binding();
        overflow.order_reward = u64::MAX;
        assert_eq!(
            required_candidate_funding_v1(overflow, 2, 0, rent(), rent(), rent()),
            Err(CodecError::ArithmeticOverflow)
        );
    }

    #[test]
    fn terminal_cleanup_protects_working_and_selected_feed_owners() {
        let node = verified_node();
        let window_identity = id(90);
        let feed = id(91);
        let working = working_best_window(node);
        let protected_view = CandidateNodeCleanupViewV1 {
            node: &node,
            window_identity,
            window: &working,
            epoch_generation: 1,
            epoch_candidate_bundle_count: 1,
            epoch_selected_candidate_count: 0,
            current_slot: 40,
            derived_feed: feed,
            derived_previous_node: Id32::ZERO,
            authenticated_feed: feed,
            authenticated_work: Id32::ZERO,
            selected: None,
        };
        let before = protected_view;
        assert_eq!(
            protected_view.classify(),
            Ok(CandidateNodeCleanupDecisionV1 {
                disposition: CandidateNodeCleanupDispositionV1::ProtectedWorkingBest,
                epoch_candidate_bundle_count_after: 1,
                window_live_node_count_after: 1,
                window_closed_node_count_after: 0,
                window_head_after: node.node,
            })
        );
        assert_eq!(protected_view, before, "classification is read-only");
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                authenticated_feed: id(92),
                ..protected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                authenticated_feed: Id32::ZERO,
                ..protected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );

        let mut refused_node = node;
        refused_node.rank_key = [0; SCORE_V2_Q_RANK_CAPACITY];
        refused_node.rank_key_len = 0;
        refused_node.status = AdmissionNodeStatusV1::VerifiedRefused;
        let mut no_best = working;
        no_best.best_candidate_node = Id32::ZERO;
        no_best.best_settlement_candidate_id = Id32::ZERO;
        no_best.best_rank_key = [0; SCORE_V2_Q_RANK_CAPACITY];
        no_best.best_ordinal = 0;
        no_best.valid_verdict_count = 0;
        let close_nonselected = CandidateNodeCleanupViewV1 {
            node: &refused_node,
            window: &no_best,
            ..protected_view
        };
        assert_eq!(
            close_nonselected.classify(),
            Ok(CandidateNodeCleanupDecisionV1 {
                disposition: CandidateNodeCleanupDispositionV1::CloseNodeAndFeed,
                epoch_candidate_bundle_count_after: 0,
                window_live_node_count_after: 0,
                window_closed_node_count_after: 1,
                window_head_after: Id32::ZERO,
            })
        );
        let close_decision = close_nonselected.classify().unwrap();
        let mut closed_window = no_best;
        closed_window.live_node_count = close_decision.window_live_node_count_after;
        closed_window.closed_node_count = close_decision.window_closed_node_count_after;
        closed_window.admission_head = close_decision.window_head_after;
        assert_eq!(closed_window.validate(), Ok(()));
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                authenticated_work: id(93),
                ..close_nonselected
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );

        let selected_artifact = id(80);
        let mut finalized = working;
        finalized.finalized_slot = 40;
        finalized.selected_candidate_artifact = selected_artifact;
        finalized.best_candidate_node = Id32::ZERO;
        finalized.best_settlement_candidate_id = Id32::ZERO;
        finalized.best_rank_key = [0; SCORE_V2_Q_RANK_CAPACITY];
        finalized.best_ordinal = 0;
        let mut selected = selected_candidate();
        selected.epoch = node.epoch;
        selected.market = node.market;
        selected.window = window_identity;
        selected.source_admission_node = node.node;
        selected.selected_feed = feed;
        selected.candidate_bundle_digest = node.candidate_bundle_digest;
        selected.settlement_candidate_id = node.settlement_candidate_id;
        selected.base_relation_candidate_id = node.base_relation_candidate_id;
        selected.settlement_witness_digest = node.settlement_witness_digest;
        selected.relation_policy_id = node.relation_policy_id;
        selected.score_policy_id = node.score_policy_id;
        selected.solver_reward_destination = node.solver_reward_destination;
        selected.rank_key = node.rank_key;
        selected.ordinal = node.ordinal;
        selected.selected_slot = 40;
        let selected_view = CandidateNodeCleanupViewV1 {
            window: &finalized,
            epoch_selected_candidate_count: 1,
            selected: Some(AuthenticatedSelectedCandidateV1 {
                artifact: selected_artifact,
                account: &selected,
            }),
            ..protected_view
        };
        assert_eq!(
            selected_view.classify(),
            Ok(CandidateNodeCleanupDecisionV1 {
                disposition: CandidateNodeCleanupDispositionV1::CloseNodeRetainingSelectedFeed,
                epoch_candidate_bundle_count_after: 0,
                window_live_node_count_after: 0,
                window_closed_node_count_after: 1,
                window_head_after: Id32::ZERO,
            })
        );
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                selected: Some(AuthenticatedSelectedCandidateV1 {
                    artifact: id(81),
                    account: &selected,
                }),
                ..selected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );

        let retired_selected = CandidateNodeCleanupViewV1 {
            epoch_selected_candidate_count: 0,
            authenticated_feed: Id32::ZERO,
            selected: None,
            ..selected_view
        };
        assert_eq!(
            retired_selected.classify(),
            Ok(CandidateNodeCleanupDecisionV1 {
                disposition: CandidateNodeCleanupDispositionV1::CloseNodeAfterFeedAbsent,
                epoch_candidate_bundle_count_after: 0,
                window_live_node_count_after: 0,
                window_closed_node_count_after: 1,
                window_head_after: Id32::ZERO,
            })
        );
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                epoch_candidate_bundle_count: 2,
                ..protected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                current_slot: 39,
                ..protected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
        let mut earlier_terminal_node = node;
        earlier_terminal_node.terminal_slot = 39;
        assert_eq!(earlier_terminal_node.validate(), Ok(()));
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                node: &earlier_terminal_node,
                current_slot: 39,
                ..selected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                derived_previous_node: id(94),
                ..protected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
        let mut wrong_kind = selected;
        wrong_kind.candidate_kind = SettlementCandidateKindV1::CoveredDealer;
        assert_eq!(wrong_kind.validate(), Ok(()));
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                selected: Some(AuthenticatedSelectedCandidateV1 {
                    artifact: selected_artifact,
                    account: &wrong_kind,
                }),
                ..selected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
        let mut wrong_selection_slot = selected;
        wrong_selection_slot.selected_slot = 41;
        assert_eq!(wrong_selection_slot.validate(), Ok(()));
        assert_eq!(
            CandidateNodeCleanupViewV1 {
                selected: Some(AuthenticatedSelectedCandidateV1 {
                    artifact: selected_artifact,
                    account: &wrong_selection_slot,
                }),
                ..selected_view
            }
            .classify(),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn active_width_headers_require_their_exact_full_frames() {
        const FEED_LEN: usize = CANDIDATE_FEED_HEADER_BYTES + (3 * 8) + 8 + 24;
        let feed = feed_header();
        let mut feed_bytes = [0u8; FEED_LEN];
        feed.encode(&mut feed_bytes[..CANDIDATE_FEED_HEADER_BYTES], true)
            .unwrap();
        let prices_at = CANDIDATE_FEED_HEADER_BYTES;
        feed_bytes[prices_at..prices_at + 8].copy_from_slice(&20u64.to_le_bytes());
        feed_bytes[prices_at + 8..prices_at + 16].copy_from_slice(&30u64.to_le_bytes());
        feed_bytes[prices_at + 16..prices_at + 24].copy_from_slice(&50u64.to_le_bytes());
        let atom_at = prices_at + (3 * 8) + 8;
        feed_bytes[atom_at..atom_at + 16].copy_from_slice(&5u128.to_le_bytes());
        feed_bytes[atom_at + 16..atom_at + 24].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(
            CandidateFeedHeaderV2::decode_account(&feed_bytes, true),
            Ok(feed)
        );
        assert_eq!(
            CandidateFeedHeaderV2::decode_account(&feed_bytes[..FEED_LEN - 1], true),
            Err(CodecError::WrongLength)
        );
        let mut wrong_sum = feed_bytes;
        wrong_sum[prices_at] = 19;
        assert_eq!(
            CandidateFeedHeaderV2::decode_account(&wrong_sum, true),
            Err(CodecError::MismatchedBinding)
        );
        let mut non_primitive = feed_bytes;
        non_primitive[atom_at + 16..atom_at + 24].copy_from_slice(&2u64.to_le_bytes());
        assert_eq!(
            CandidateFeedHeaderV2::decode_account(&non_primitive, true),
            Err(CodecError::MismatchedBinding)
        );

        let mut stage = feed;
        stage.prices_written = 2;
        stage.fills_written = 0;
        stage.atoms_written = 0;
        let mut stage_bytes = [0u8; FEED_LEN];
        stage
            .encode(&mut stage_bytes[..CANDIDATE_FEED_HEADER_BYTES], false)
            .unwrap();
        stage_bytes[prices_at..prices_at + 8].copy_from_slice(&20u64.to_le_bytes());
        stage_bytes[prices_at + 8..prices_at + 16].copy_from_slice(&30u64.to_le_bytes());
        assert_eq!(
            CandidateFeedHeaderV2::decode_account(&stage_bytes, false),
            Ok(stage)
        );
        stage_bytes[prices_at + 16] = 1;
        assert_eq!(
            CandidateFeedHeaderV2::decode_account(&stage_bytes, false),
            Err(CodecError::NonCanonicalPadding)
        );

        const WORK_LEN: usize = CLEAR_WORK_HEADER_BYTES + (3 * 16) + (3 * 8);
        let work = work_header();
        let mut work_bytes = [0u8; WORK_LEN];
        work.encode(&mut work_bytes[..CLEAR_WORK_HEADER_BYTES])
            .unwrap();
        assert_eq!(ClearWorkHeaderV2::decode_account(&work_bytes), Ok(work));
        assert_eq!(
            ClearWorkHeaderV2::decode_account(&work_bytes[..WORK_LEN - 1]),
            Err(CodecError::WrongLength)
        );
        let mut invalid_phase = work;
        invalid_phase.phase = 2;
        assert_eq!(invalid_phase.validate(), Err(CodecError::InvalidState));

        const WORK_V3_LEN: usize = CLEAR_WORK_V3_HEADER_BYTES + (3 * 16) + (3 * 8);
        let work_v3 = work_v3_header();
        let mut work_v3_bytes = [0u8; WORK_V3_LEN];
        work_v3
            .encode(&mut work_v3_bytes[..CLEAR_WORK_V3_HEADER_BYTES])
            .unwrap();
        assert_eq!(
            ClearWorkV3AccountV1::decode_account(&work_v3_bytes),
            Ok(work_v3)
        );
        assert_eq!(
            ClearWorkV3AccountV1::decode_account(&work_v3_bytes[..WORK_V3_LEN - 1]),
            Err(CodecError::WrongLength)
        );
        work_v3_bytes[CLEAR_WORK_V3_HEADER_BYTES] = 1;
        assert_eq!(
            ClearWorkV3AccountV1::decode_account(&work_v3_bytes),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut false_verdict = work_v3;
        false_verdict.verification_state = ClearWorkVerificationStateV1::Valid;
        assert_eq!(false_verdict.validate(), Err(CodecError::InvalidState));
        assert_eq!(
            ClearWorkHeaderV2::decode_account(&work_v3_bytes),
            Err(CodecError::WrongVersion)
        );

        let mut folded_v3 = work_v3;
        folded_v3.page_count = 1;
        folded_v3.phase = 1;
        folded_v3.order_cursor = 1;
        folded_v3.previous_order_id = id(41);
        let mut folded_v3_bytes = [0u8; WORK_V3_LEN];
        folded_v3
            .encode(&mut folded_v3_bytes[..CLEAR_WORK_V3_HEADER_BYTES])
            .unwrap();
        folded_v3_bytes[CLEAR_WORK_V3_HEADER_BYTES..CLEAR_WORK_V3_HEADER_BYTES + 8]
            .copy_from_slice(&7u64.to_le_bytes());
        folded_v3_bytes[CLEAR_WORK_V3_HEADER_BYTES + 8..CLEAR_WORK_V3_HEADER_BYTES + 16]
            .copy_from_slice(&3u64.to_le_bytes());
        let row_at = CLEAR_WORK_V3_HEADER_BYTES + (3 * 16);
        folded_v3_bytes[row_at..row_at + 8].copy_from_slice(&5u64.to_le_bytes());
        assert_eq!(
            decode_clear_work_v3_relation_flows(&folded_v3_bytes),
            Ok(ClearWorkRelationFlowsV1 {
                aggregate_buy_flow: [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                aggregate_sell_flow: [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            })
        );
        assert_eq!(
            decode_clear_work_v3_filled_legs(&folded_v3_bytes, 0),
            Ok([5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        );
        assert_eq!(
            decode_clear_work_v3_filled_legs(&folded_v3_bytes, 1),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn feed_digest_owners_bind_exact_active_tails_without_cycles() {
        const FEED_LEN: usize = CANDIDATE_FEED_HEADER_BYTES + (3 * 8) + 8 + 24;
        let feed = feed_header();
        let mut bytes = [0u8; FEED_LEN];
        feed.encode(&mut bytes[..CANDIDATE_FEED_HEADER_BYTES], true)
            .unwrap();
        let prices_at = CANDIDATE_FEED_HEADER_BYTES;
        bytes[prices_at..prices_at + 8].copy_from_slice(&20u64.to_le_bytes());
        bytes[prices_at + 8..prices_at + 16].copy_from_slice(&30u64.to_le_bytes());
        bytes[prices_at + 16..prices_at + 24].copy_from_slice(&50u64.to_le_bytes());
        let fills_at = prices_at + 24;
        let atoms_at = fills_at + 8;
        bytes[atoms_at..atoms_at + 16].copy_from_slice(&5u128.to_le_bytes());
        bytes[atoms_at + 16..atoms_at + 24].copy_from_slice(&1u64.to_le_bytes());

        let tail = candidate_feed_tail_v2(&bytes, feed).unwrap();
        let offsets = candidate_feed_tail_offsets_v2(feed).unwrap();
        assert_eq!(offsets.prices_at(), prices_at);
        assert_eq!(offsets.fills_at(), fills_at);
        assert_eq!(offsets.atoms_at(), atoms_at);
        assert_eq!(offsets.slices_at(), atoms_at + 24);
        assert_eq!(offsets.end(), FEED_LEN);
        assert_eq!(tail.prices_le().len(), 24);
        assert_eq!(tail.fills_le().len(), 8);
        assert_eq!(tail.atoms_le().len(), 24);
        assert!(tail.slices_le().is_empty());
        let witness = quantized_witness_body_digest_v3(&Sha2Backend, id(30), &bytes, true).unwrap();
        let bundle = candidate_bundle_digest_v1(&Sha2Backend, &bytes, true).unwrap();

        let mut changed_price = bytes;
        changed_price[prices_at..prices_at + 8].copy_from_slice(&21u64.to_le_bytes());
        changed_price[prices_at + 8..prices_at + 16].copy_from_slice(&29u64.to_le_bytes());
        assert_ne!(
            witness,
            quantized_witness_body_digest_v3(&Sha2Backend, id(30), &changed_price, true).unwrap()
        );
        assert_ne!(
            bundle,
            candidate_bundle_digest_v1(&Sha2Backend, &changed_price, true).unwrap()
        );

        let mut changed_fill = bytes;
        changed_fill[fills_at..fills_at + 8].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(
            witness,
            quantized_witness_body_digest_v3(&Sha2Backend, id(30), &changed_fill, true).unwrap(),
            "the V3 witness body excludes relation fills"
        );
        assert_ne!(
            bundle,
            candidate_bundle_digest_v1(&Sha2Backend, &changed_fill, true).unwrap()
        );
        assert_ne!(
            empty_settlement_witness_digest_v1(&Sha2Backend, id(40)).unwrap(),
            empty_settlement_witness_digest_v1(&Sha2Backend, id(41)).unwrap()
        );
        assert_eq!(
            candidate_bundle_digest_v1(&Sha2Backend, &bytes[..FEED_LEN - 1], true),
            Err(CodecError::WrongLength)
        );
    }

    #[test]
    fn active_width_formulas_refuse_every_out_of_range_coordinate() {
        assert_eq!(
            candidate_feed_account_len(1, 1, 1, 0),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            candidate_feed_account_len(17, 1, 1, 0),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            candidate_feed_account_len(2, 65, 1, 0),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            candidate_feed_account_len(2, 1, 0, 0),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            candidate_feed_account_len(2, 1, 3, 0),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            candidate_feed_account_len(2, 1, 1, 417),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(clear_work_account_len(1, 0), Err(CodecError::InvalidCount));
        assert_eq!(clear_work_account_len(17, 0), Err(CodecError::InvalidCount));
        assert_eq!(clear_work_account_len(2, 65), Err(CodecError::InvalidCount));
    }

    #[test]
    fn rank_preserves_score_v2_digest_then_prefers_first_admitted_duplicate() {
        let score = ScoreV2QComponentsV1 {
            certified_risk_flow_atoms: 7,
            cash_equivalent_direct_flow_atoms: 3,
            virtual_churn_atoms: 2,
            settlement_candidate_id: id(4),
        };
        let first =
            encode_score_v2_q_first_admitted_tie_v1(score, FirstAdmittedTieV1 { ordinal: 1 })
                .unwrap();
        let later =
            encode_score_v2_q_first_admitted_tie_v1(score, FirstAdmittedTieV1 { ordinal: 2 })
                .unwrap();
        assert!(first > later);
        assert!(first[88..].iter().all(|b| *b == 0));
        let mut smaller = score;
        smaller.settlement_candidate_id = id(3);
        let a =
            encode_score_v2_q_first_admitted_tie_v1(smaller, FirstAdmittedTieV1 { ordinal: 99 })
                .unwrap();
        assert!(
            a > first,
            "smaller frozen ScoreV2 digest wins before ordinal"
        );
        assert_eq!(
            encode_score_v2_q_first_admitted_tie_v1(score, FirstAdmittedTieV1 { ordinal: 0 }),
            Err(CodecError::InvalidState)
        );
        let mut zero_id = score;
        zero_id.settlement_candidate_id = Id32::ZERO;
        assert_eq!(
            encode_score_v2_q_first_admitted_tie_v1(zero_id, FirstAdmittedTieV1 { ordinal: 1 }),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[test]
    fn sha_checkpoint_requires_zero_padding_and_length_congruence() {
        let mut value = Sha256CheckpointV1 {
            state: [1; 8],
            block: [0; 64],
            block_len: 1,
            total_len: 1,
        };
        value.block[0] = 7;
        assert_eq!(value.validate(), Ok(()));
        value.block[2] = 1;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        value.block[2] = 0;
        value.total_len = 2;
        assert_eq!(value.validate(), Err(CodecError::InvalidState));
    }

    #[test]
    fn economic_domain_transcript_is_exact_and_binds_full_market_instance() {
        let value = EconomicDomainV2Transcript {
            relation_version: 2,
            market_instance_v2_id: id(1),
            epoch_semantics_digest: id(2),
            relation_policy_id: id(3),
            price_measure_policy_v1_id: id(4),
            native_claim_basis_id: id(5),
            epoch_index: 7,
            outcome_count: 3,
            price_scale: 100,
            coordinate_domain_min: 10,
            coordinate_domain_max: 1_000,
        };
        let encoded = value.encode().unwrap();
        assert_eq!(encoded.len(), 213);
        assert_eq!(&encoded[4..36], &[1; 32]);
        let mut changed = value;
        changed.market_instance_v2_id = id(9);
        assert_ne!(encoded, changed.encode().unwrap());
        assert_eq!(
            economic_domain_digest_v2(&LengthSha, value)
                .unwrap()
                .bytes()[0],
            u8::try_from(
                ECONOMIC_DOMAIN_DIGEST_DOMAIN_V1.len() + ECONOMIC_DOMAIN_V2_TRANSCRIPT_BYTES
            )
            .unwrap()
        );

        let account = EconomicDomainV2AccountV1 {
            epoch: id(6),
            transcript: value,
            rent: rent(),
            stored_bump: 1,
            flags: 0,
        };
        let mut bytes = [0u8; ECONOMIC_DOMAIN_ACCOUNT_BYTES];
        account.encode(&mut bytes).unwrap();
        assert_eq!(EconomicDomainV2AccountV1::decode(&bytes), Ok(account));
        bytes[ECONOMIC_DOMAIN_ACCOUNT_BYTES - 1] = 1;
        assert_eq!(
            EconomicDomainV2AccountV1::decode(&bytes),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn price_semantics_digest_is_relation_v2_byte_exact_at_both_width_extremes() {
        fn relation_domain(
            domain: EconomicDomainV2Transcript,
        ) -> clutch_batch::relation_v2::EconomicDomainV2 {
            clutch_batch::relation_v2::EconomicDomainV2 {
                relation_version: domain.relation_version,
                market_semantics_digest: domain.market_instance_v2_id.bytes(),
                epoch_semantics_digest: domain.epoch_semantics_digest.bytes(),
                relation_policy_digest: domain.relation_policy_id.bytes(),
                price_policy_digest: domain.price_measure_policy_v1_id.bytes(),
                epoch_index: domain.epoch_index,
                outcome_count: domain.outcome_count,
                price_scale: domain.price_scale,
            }
        }
        fn assert_equal(domain: EconomicDomainV2Transcript, prices: [u64; MAX_OUTCOMES]) -> Id32 {
            let general =
                price_semantics_digest_v2(&Sha2Backend, PriceSemanticsV2 { domain, prices })
                    .unwrap();
            let relation = clutch_batch::relation_v2::price_semantics_digest_v2(
                &relation_domain(domain),
                &prices,
            )
            .unwrap();
            assert_eq!(general.bytes(), relation);
            general
        }

        let prices_two = [40, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let domain_two = economic_domain(2, 100);
        let baseline = assert_equal(domain_two, prices_two);

        let mut prices_sixteen = [0u64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            prices_sixteen[index] = u64::try_from(index + 1).unwrap();
            index += 1;
        }
        assert_equal(economic_domain(16, 136), prices_sixteen);

        for changed in [
            EconomicDomainV2Transcript {
                market_instance_v2_id: id(9),
                ..domain_two
            },
            EconomicDomainV2Transcript {
                epoch_semantics_digest: id(9),
                ..domain_two
            },
            EconomicDomainV2Transcript {
                price_measure_policy_v1_id: id(9),
                ..domain_two
            },
            EconomicDomainV2Transcript {
                epoch_index: 8,
                ..domain_two
            },
        ] {
            assert_ne!(assert_equal(changed, prices_two), baseline);
        }

        let mut padded = prices_two;
        padded[2] = 1;
        assert_eq!(
            price_semantics_digest_v2(
                &Sha2Backend,
                PriceSemanticsV2 {
                    domain: domain_two,
                    prices: padded,
                },
            ),
            Err(CodecError::NonCanonicalPadding)
        );
        assert!(clutch_batch::relation_v2::price_semantics_digest_v2(
            &relation_domain(domain_two),
            &padded,
        )
        .is_err());

        let mut wrong_sum = prices_two;
        wrong_sum[1] = 59;
        assert_eq!(
            price_semantics_digest_v2(
                &Sha2Backend,
                PriceSemanticsV2 {
                    domain: domain_two,
                    prices: wrong_sum,
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
        assert!(clutch_batch::relation_v2::price_semantics_digest_v2(
            &relation_domain(domain_two),
            &wrong_sum,
        )
        .is_err());
    }

    #[test]
    fn counted_bundle_and_window_head_must_retire_together() {
        let live = CandidateBundleRetirementContractV1 {
            epoch_generation: 1,
            epoch_candidate_bundle_count: 1,
            epoch_selected_candidate_count: 0,
            window_live_node_count: 1,
            window_head: id(1),
        };
        assert_eq!(live.retired(), Ok(false));
        let retired = CandidateBundleRetirementContractV1 {
            epoch_generation: 1,
            epoch_candidate_bundle_count: 0,
            epoch_selected_candidate_count: 0,
            window_live_node_count: 0,
            window_head: Id32::ZERO,
        };
        assert_eq!(retired.retired(), Ok(true));
        let mismatch = CandidateBundleRetirementContractV1 {
            epoch_candidate_bundle_count: 0,
            ..live
        };
        assert_eq!(mismatch.retired(), Err(CodecError::MismatchedBinding));
        let selected_live = CandidateBundleRetirementContractV1 {
            epoch_generation: 1,
            epoch_candidate_bundle_count: 0,
            epoch_selected_candidate_count: 1,
            window_live_node_count: 0,
            window_head: Id32::ZERO,
        };
        assert_eq!(selected_live.retired(), Ok(false));
        let corrupt_selected_count = CandidateBundleRetirementContractV1 {
            epoch_selected_candidate_count: 2,
            ..selected_live
        };
        assert_eq!(
            corrupt_selected_count.retired(),
            Err(CodecError::InvalidState)
        );

        let materializing = SelectedCandidateRetirementContractV1 {
            epoch: id(1),
            epoch_generation: 1,
            epoch_selected_candidate_count: 1,
            artifact: id(2),
            artifact_epoch: id(1),
            artifact_epoch_generation: 1,
            window_selected_artifact: id(2),
            artifact_window: id(9),
            derived_window: id(9),
            authenticated_window: id(9),
            window_epoch: id(1),
            window_epoch_generation: 1,
            retained_feed: id(3),
            authenticated_feed: id(3),
            feed_epoch: id(1),
            feed_epoch_generation: 1,
            feed_slice_count: 2,
            derived_budget: id(4),
            authenticated_budget: id(4),
            budget_epoch: id(1),
            budget_epoch_generation: 1,
            budget_funding_payer: id(7),
            artifact_rent_payer: id(7),
            budget_selected_rent_state: 1,
            budget_selected_rent_remaining: 0,
            budget_selected_rent_initial: 14,
            artifact_rent_refundable_principal: 14,
            slice_count: 2,
            next_slice_index: 1,
            entitlement_state: 1,
            budget_solver_state: 1,
        };
        assert_eq!(materializing.retirable(), Ok(false));
        let materialized = SelectedCandidateRetirementContractV1 {
            next_slice_index: 2,
            entitlement_state: 2,
            ..materializing
        };
        assert_eq!(materialized.retirable(), Ok(true));
        assert_eq!(
            SelectedCandidateRetirementContractV1 {
                feed_slice_count: 0,
                slice_count: 0,
                next_slice_index: 0,
                entitlement_state: 0,
                ..materializing
            }
            .retirable(),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            SelectedCandidateRetirementContractV1 {
                feed_slice_count: 0,
                slice_count: 0,
                next_slice_index: 0,
                entitlement_state: 2,
                ..materializing
            }
            .retirable(),
            Ok(true)
        );
        let wrong_feed = SelectedCandidateRetirementContractV1 {
            authenticated_feed: id(4),
            ..materialized
        };
        assert_eq!(wrong_feed.retirable(), Err(CodecError::MismatchedBinding));
        let solver_open = SelectedCandidateRetirementContractV1 {
            budget_solver_state: 0,
            ..materialized
        };
        assert_eq!(solver_open.retirable(), Err(CodecError::InvalidState));
        let wrong_budget = SelectedCandidateRetirementContractV1 {
            authenticated_budget: id(5),
            ..materialized
        };
        assert_eq!(wrong_budget.retirable(), Err(CodecError::MismatchedBinding));
        let wrong_budget_epoch = SelectedCandidateRetirementContractV1 {
            budget_epoch: id(6),
            ..materialized
        };
        assert_eq!(
            wrong_budget_epoch.retirable(),
            Err(CodecError::MismatchedBinding)
        );
        let stale_feed = SelectedCandidateRetirementContractV1 {
            feed_epoch_generation: 2,
            ..materialized
        };
        assert_eq!(stale_feed.retirable(), Err(CodecError::InvalidState));
        let wrong_feed_slices = SelectedCandidateRetirementContractV1 {
            feed_slice_count: 1,
            ..materialized
        };
        assert_eq!(
            wrong_feed_slices.retirable(),
            Err(CodecError::MismatchedBinding)
        );
        let wrong_rent_payer = SelectedCandidateRetirementContractV1 {
            artifact_rent_payer: id(8),
            ..materialized
        };
        assert_eq!(
            wrong_rent_payer.retirable(),
            Err(CodecError::MismatchedBinding)
        );
        let rent_still_open = SelectedCandidateRetirementContractV1 {
            budget_selected_rent_state: 0,
            ..materialized
        };
        assert_eq!(rent_still_open.retirable(), Err(CodecError::InvalidState));
        let rent_still_present = SelectedCandidateRetirementContractV1 {
            budget_selected_rent_remaining: 1,
            ..materialized
        };
        assert_eq!(
            rent_still_present.retirable(),
            Err(CodecError::InvalidState)
        );
        let wrong_rent_principal = SelectedCandidateRetirementContractV1 {
            artifact_rent_refundable_principal: 15,
            ..materialized
        };
        assert_eq!(
            wrong_rent_principal.retirable(),
            Err(CodecError::MismatchedBinding)
        );
        let wrong_window = SelectedCandidateRetirementContractV1 {
            authenticated_window: id(10),
            ..materialized
        };
        assert_eq!(wrong_window.retirable(), Err(CodecError::MismatchedBinding));
        let wrong_window_epoch = SelectedCandidateRetirementContractV1 {
            window_epoch: id(11),
            ..materialized
        };
        assert_eq!(
            wrong_window_epoch.retirable(),
            Err(CodecError::MismatchedBinding)
        );
    }
}
