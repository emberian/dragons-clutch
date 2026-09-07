//! The parent reference a child market founds against, and the derived
//! provider's settle request.
//!
//! The semantics are `DClutchSemantics.ConditionalMarketV1` (fifty theorems:
//! a product `A × B` is `R_A · R_B` cells row-major, a conditional `B | A = a`
//! is the product's row projection with one off-condition cell, both settle to
//! their own failure coordinate exactly when a parent the branch depends on
//! failed).  The wire is `DClutchSemantics.ParentReferenceV1Abi`; every offset
//! and identity here is its emitted twin.  This module is the Rust that reads
//! those bytes and computes those functions, and nothing else: it hashes
//! nothing, touches no account, and knows no program.
//!
//! # Where the reference is named
//!
//! By `SourceSpecV1::adapter_config_id`, the per-market adapter-configuration
//! slot, exactly where `PythAdapterConfigV1` lives for a Pyth market.  The
//! child's `ProviderReleaseV1` is the derived family's constant release; its
//! `decoding_rules_id` is one rule ("the joint index over denominator one at
//! exponent zero") and never varies per market.  The design note's §2.1 named
//! the reference through `decoding_rules_id`; the ABI module's header records
//! the correction and why.

use core::convert::TryInto;

use crate::generated_parent_reference_v1 as generated;
use crate::{ContentId, Error, Result};

pub use generated::{
    DERIVED_ACCESS_PROFILE_V1, DERIVED_DECODING_RULES_ID_V1, DERIVED_DECODING_RULES_PREIMAGE_V1,
    DERIVED_EVIDENCE_DOMAIN_V1, DERIVED_FAMILY_RELEASE_ID_V1, DERIVED_FAMILY_RELEASE_PREIMAGE_V1,
    DERIVED_PROVIDER_EXTENSION_PREIMAGE_V1, DERIVED_PROVIDER_EXTENSION_RELEASE_ID_V1,
    DERIVED_SETTLE_ACCOUNT_COUNT_V1, DERIVED_SETTLE_ACCOUNT_ROLES_V1, DERIVED_SETTLE_ACTION_V1,
    DERIVED_SETTLE_CERTIFICATE_INDEX_V1, DERIVED_SETTLE_CLOCK_INDEX_V1, DERIVED_SETTLE_MAGIC_V1,
    DERIVED_SETTLE_MARKET_INDEX_V1, DERIVED_SETTLE_MATERIAL_RAW_INDEX_V1,
    DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1, DERIVED_SETTLE_PARENT_A_MARKET_INDEX_V1,
    DERIVED_SETTLE_PARENT_B_MARKET_INDEX_V1, DERIVED_SETTLE_PARENT_REFERENCE_RAW_INDEX_V1,
    DERIVED_SETTLE_PARENT_SLOT_COUNT_V1, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1,
    DERIVED_SETTLE_RENT_INDEX_V1, DERIVED_SETTLE_REQUEST_BYTES_V1,
    DERIVED_SETTLE_SOURCE_STATE_INDEX_V1, DERIVED_SETTLE_SYSTEM_INDEX_V1,
    DERIVED_SETTLE_VERSION_V1, DERIVED_TRANSPORT_PROFILE_ID_V1,
    DERIVED_TRANSPORT_PROFILE_PREIMAGE_V1, PARENT_REFERENCE_BYTES_V1,
    PARENT_REFERENCE_KIND_CONDITIONAL_V1, PARENT_REFERENCE_KIND_PRODUCT_V1,
    PARENT_REFERENCE_MAGIC_V1, PARENT_REFERENCE_PARENT_STRIDE_V1, PARENT_REFERENCE_SCHEMA_ID_V1,
    PARENT_REFERENCE_SCHEMA_PREIMAGE_V1, PARENT_REFERENCE_VERSION_V1,
};

/// The General bank cap, `151 + 6K <= 512 => K <= 60`
/// (`programs/dclutch-trading-sbf/src/hot_v3.rs:456`); `maxOutcomeCount` in
/// the Lean. MATHEMATICAL, and restated here as a mirror pinned by
/// `the_bank_cap_matches_the_lean` below rather than derived, because this
/// crate does not link Trading.
pub const CHILD_MAXIMUM_OUTCOME_COUNT_V1: u32 = 60;

/// Which child shape a reference founds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParentReferenceKindV1 {
    /// `A × B`: `R_A · R_B` ordinary cells, row-major with `A` major.
    Product,
    /// `B | A = condition`: `R_B` branch cells, one off-condition cell.
    Conditional,
}

impl ParentReferenceKindV1 {
    fn decode(byte: u8) -> Result<Self> {
        if byte == PARENT_REFERENCE_KIND_PRODUCT_V1 {
            Ok(Self::Product)
        } else if byte == PARENT_REFERENCE_KIND_CONDITIONAL_V1 {
            Ok(Self::Conditional)
        } else {
            Err(Error::UnsupportedSchema)
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Product => PARENT_REFERENCE_KIND_PRODUCT_V1,
            Self::Conditional => PARENT_REFERENCE_KIND_CONDITIONAL_V1,
        }
    }
}

/// One parent as the child binds it at founding: the twin of the Lean
/// `ParentRef`.  A parent replaced after founding — a new generation at the
/// same address, or a re-founded Product — fails this at settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentRefV1 {
    /// The parent's Core Market account.
    pub market: [u8; 32],
    /// The parent's immutable generation.
    pub generation: u64,
    /// The parent's Product-record content digest.
    pub product_record_digest: [u8; 32],
    /// The parent's `ResultDomainV2::region_count`; its failure selector.
    pub ordinary_count: u32,
}

/// One parent group's field offsets, relative to that parent's own base.
///
/// The Lean emits both parents' ABSOLUTE offsets; the two groups are one
/// layout at one stride, so the deltas are read off parent A and the const
/// assertions below say parent B is that same group shifted by the stride. A
/// field inserted into the group moves both parents in the emitter, and any
/// disagreement between the two is a compile error here rather than a
/// misparse at settlement.
const PARENT_GENERATION_DELTA_V1: usize = generated::PARENT_REFERENCE_PARENT_A_GENERATION_OFFSET_V1
    - generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1;
const PARENT_RECORD_DELTA_V1: usize = generated::PARENT_REFERENCE_PARENT_A_RECORD_OFFSET_V1
    - generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1;
const PARENT_ORDINARY_DELTA_V1: usize = generated::PARENT_REFERENCE_PARENT_A_ORDINARY_OFFSET_V1
    - generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1;
const PARENT_RESERVED_DELTA_V1: usize = generated::PARENT_REFERENCE_RESERVED_A_OFFSET_V1
    - generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1;
/// The canonical-zero span closing a parent group, to the next stride.
const PARENT_RESERVED_BYTES_V1: usize =
    PARENT_REFERENCE_PARENT_STRIDE_V1 - PARENT_RESERVED_DELTA_V1;

const _: () = assert!(
    generated::PARENT_REFERENCE_PARENT_B_MARKET_OFFSET_V1
        == generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1
            + PARENT_REFERENCE_PARENT_STRIDE_V1
        && generated::PARENT_REFERENCE_PARENT_B_GENERATION_OFFSET_V1
            == generated::PARENT_REFERENCE_PARENT_A_GENERATION_OFFSET_V1
                + PARENT_REFERENCE_PARENT_STRIDE_V1
        && generated::PARENT_REFERENCE_PARENT_B_RECORD_OFFSET_V1
            == generated::PARENT_REFERENCE_PARENT_A_RECORD_OFFSET_V1
                + PARENT_REFERENCE_PARENT_STRIDE_V1
        && generated::PARENT_REFERENCE_PARENT_B_ORDINARY_OFFSET_V1
            == generated::PARENT_REFERENCE_PARENT_A_ORDINARY_OFFSET_V1
                + PARENT_REFERENCE_PARENT_STRIDE_V1
        && generated::PARENT_REFERENCE_RESERVED_B_OFFSET_V1
            == generated::PARENT_REFERENCE_RESERVED_A_OFFSET_V1 + PARENT_REFERENCE_PARENT_STRIDE_V1,
    "the two parent groups are not one layout at one stride"
);

impl ParentRefV1 {
    fn validate(self) -> Result<()> {
        if is_zero(&self.market) || is_zero(&self.product_record_digest) {
            return Err(Error::ZeroContentId);
        }
        if self.generation == 0 || self.ordinary_count == 0 {
            return Err(Error::ZeroIdentifier);
        }
        Ok(())
    }

    fn field(base: usize, delta: usize) -> Result<usize> {
        base.checked_add(delta).ok_or(Error::ArithmeticOverflow)
    }

    fn decode(bytes: &[u8], base: usize) -> Result<Self> {
        let value = Self {
            market: array(bytes, base)?,
            generation: read_u64(bytes, Self::field(base, PARENT_GENERATION_DELTA_V1)?)?,
            product_record_digest: array(bytes, Self::field(base, PARENT_RECORD_DELTA_V1)?)?,
            ordinary_count: read_u32(bytes, Self::field(base, PARENT_ORDINARY_DELTA_V1)?)?,
        };
        require_zero(
            bytes,
            Self::field(base, PARENT_RESERVED_DELTA_V1)?,
            PARENT_RESERVED_BYTES_V1,
        )?;
        value.validate()?;
        Ok(value)
    }

    fn encode_into(self, output: &mut [u8], base: usize) {
        put(output, base, &self.market);
        put(
            output,
            base + PARENT_GENERATION_DELTA_V1,
            &self.generation.to_le_bytes(),
        );
        put(
            output,
            base + PARENT_RECORD_DELTA_V1,
            &self.product_record_digest,
        );
        put(
            output,
            base + PARENT_ORDINARY_DELTA_V1,
            &self.ordinary_count.to_le_bytes(),
        );
    }
}

/// The fields of a parent's `ResolutionCertificateV2` the child reads, plus
/// whether its kind is one of the two terminals.  The twin of the Lean
/// `ParentCertificate`.  The two liveness kinds (`RecoveryAdvanced`,
/// `Exhausted`) are not terminals and a child presented one has been presented
/// nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentCertificateV1 {
    /// `certificate.market`.
    pub market: [u8; 32],
    /// `certificate.generation`.
    pub generation: u64,
    /// `certificate.product_record_digest`.
    pub product_record_digest: [u8; 32],
    /// The parent's Product width less one, as the settling route binds it.
    pub ordinary_count: u32,
    /// `certificate.selector`, untruncated.
    pub selector: u32,
    /// `kind ∈ {ResolutionSuccess, ResolutionFailure}`.
    pub terminal: bool,
}

/// A parent's admitted terminal: the twin of the Lean `ParentTerminal`.  A
/// selector equal to the ordinary count is the parent's failure coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentTerminalV1 {
    /// The parent's ordinary count, from the reference.
    pub ordinary_count: u32,
    /// The parent's selector, from its certificate.
    pub selector: u32,
}

impl ParentTerminalV1 {
    /// Whether the parent resolved to its own failure coordinate.
    pub const fn failed(self) -> bool {
        self.selector >= self.ordinary_count
    }
}

/// The eleven refusals of `ConditionalMarketV1.Refusal`, by name.  Six are the
/// derived provider's (Resolution's band), five are founding's (Core's band);
/// each program maps these onto its own discriminants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParentRefusalV1 {
    /// A parent the branch depends on has no terminal certificate yet.
    ParentNotTerminal,
    /// The certificate names a market the reference does not.
    WrongParent,
    /// The certificate's generation is not the referenced one.
    ParentGenerationMismatch,
    /// The certificate binds a Product record the reference does not.
    ParentRecordMismatch,
    /// The parent's ordinary count is not the referenced one.
    ParentWidthMismatch,
    /// The certificate's selector exceeds the parent's width.
    SelectorOutOfRange,
    /// The condition names an outcome past the parent's failure coordinate.
    ConditionOutOfRange,
    /// The condition names the parent's failure coordinate: a market that pays
    /// on an outage, which decision 0025 forbids.
    ConditionOnFailure,
    /// Both parents are one market.
    SameParent,
    /// A parent with no ordinary outcome.
    EmptyParent,
    /// The child's width exceeds the bank cap.
    WidthOverflow,
}

/// Admit one certificate against the reference it must satisfy: the Lean
/// `ParentRef.admit`, conjunct for conjunct and in the same order, so the
/// refusal a hostile earns here is the refusal the theorem names.
pub fn admit_parent_certificate_v1(
    reference: ParentRefV1,
    certificate: ParentCertificateV1,
) -> core::result::Result<ParentTerminalV1, ParentRefusalV1> {
    if !certificate.terminal {
        return Err(ParentRefusalV1::ParentNotTerminal);
    }
    if certificate.market != reference.market {
        return Err(ParentRefusalV1::WrongParent);
    }
    if certificate.generation != reference.generation {
        return Err(ParentRefusalV1::ParentGenerationMismatch);
    }
    if certificate.product_record_digest != reference.product_record_digest {
        return Err(ParentRefusalV1::ParentRecordMismatch);
    }
    if certificate.ordinary_count != reference.ordinary_count {
        return Err(ParentRefusalV1::ParentWidthMismatch);
    }
    if reference.ordinary_count < certificate.selector {
        return Err(ParentRefusalV1::SelectorOutOfRange);
    }
    Ok(ParentTerminalV1 {
        ordinary_count: reference.ordinary_count,
        selector: certificate.selector,
    })
}

/// The decoded reference record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentReferenceV1 {
    /// Product or conditional.
    pub kind: ParentReferenceKindV1,
    /// The major axis; the condition's parent for a conditional.
    pub parent_a: ParentRefV1,
    /// The minor axis; the branch's parent for a conditional.
    pub parent_b: ParentRefV1,
    /// The ordinary outcome of `A` the branch is conditioned on; zero for a
    /// product record, and checked zero at decode.
    pub condition: u32,
    /// The child's deadline, in Unix seconds: `windowEnd = acceptThrough`.
    pub settle_by: u64,
}

impl ParentReferenceV1 {
    /// Hostile-decode one exactly 200-byte record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PARENT_REFERENCE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, generated::PARENT_REFERENCE_MAGIC_OFFSET_V1)?
            != PARENT_REFERENCE_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, generated::PARENT_REFERENCE_VERSION_OFFSET_V1)?
            != PARENT_REFERENCE_VERSION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            generated::PARENT_REFERENCE_RESERVED_HEADER_OFFSET_V1,
            5,
        )?;
        require_zero(
            bytes,
            generated::PARENT_REFERENCE_RESERVED_CONDITION_OFFSET_V1,
            4,
        )?;
        require_zero(
            bytes,
            generated::PARENT_REFERENCE_RESERVED_TAIL_OFFSET_V1,
            8,
        )?;
        let kind = ParentReferenceKindV1::decode(byte(
            bytes,
            generated::PARENT_REFERENCE_KIND_OFFSET_V1,
        )?)?;
        let value = Self {
            kind,
            parent_a: ParentRefV1::decode(
                bytes,
                generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1,
            )?,
            parent_b: ParentRefV1::decode(
                bytes,
                generated::PARENT_REFERENCE_PARENT_B_MARKET_OFFSET_V1,
            )?,
            condition: read_u32(bytes, generated::PARENT_REFERENCE_CONDITION_OFFSET_V1)?,
            settle_by: read_u64(bytes, generated::PARENT_REFERENCE_SETTLE_BY_OFFSET_V1)?,
        };
        value.validate_canonical()?;
        Ok(value)
    }

    /// Encode one exact canonical record.
    pub fn to_bytes(self) -> Result<[u8; PARENT_REFERENCE_BYTES_V1]> {
        self.parent_a.validate()?;
        self.parent_b.validate()?;
        self.validate_canonical()?;
        let mut output = [0_u8; PARENT_REFERENCE_BYTES_V1];
        put(
            &mut output,
            generated::PARENT_REFERENCE_MAGIC_OFFSET_V1,
            &PARENT_REFERENCE_MAGIC_V1,
        );
        put(
            &mut output,
            generated::PARENT_REFERENCE_VERSION_OFFSET_V1,
            &PARENT_REFERENCE_VERSION_V1.to_le_bytes(),
        );
        put(
            &mut output,
            generated::PARENT_REFERENCE_KIND_OFFSET_V1,
            &[self.kind.byte()],
        );
        self.parent_a.encode_into(
            &mut output,
            generated::PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1,
        );
        self.parent_b.encode_into(
            &mut output,
            generated::PARENT_REFERENCE_PARENT_B_MARKET_OFFSET_V1,
        );
        put(
            &mut output,
            generated::PARENT_REFERENCE_CONDITION_OFFSET_V1,
            &self.condition.to_le_bytes(),
        );
        put(
            &mut output,
            generated::PARENT_REFERENCE_SETTLE_BY_OFFSET_V1,
            &self.settle_by.to_le_bytes(),
        );
        Ok(output)
    }

    /// Canonical bytes: what the decoder accepts.  The founding conjuncts are
    /// [`Self::admit_founding`] and are deliberately not folded in here, so a
    /// record can be canonical and still be refused at founding by name.
    fn validate_canonical(self) -> Result<()> {
        if self.settle_by == 0 {
            return Err(Error::ZeroIdentifier);
        }
        if self.kind == ParentReferenceKindV1::Product && self.condition != 0 {
            return Err(Error::NonCanonicalReservedBytes);
        }
        Ok(())
    }

    /// The founding conjuncts: `ProductShape.found?` and
    /// `ConditionalShape.found?`, in the theorems' order.
    pub fn admit_founding(self) -> core::result::Result<ChildShapeV1, ParentRefusalV1> {
        if self.parent_a.market == self.parent_b.market {
            return Err(ParentRefusalV1::SameParent);
        }
        if self.parent_a.ordinary_count == 0 || self.parent_b.ordinary_count == 0 {
            return Err(ParentRefusalV1::EmptyParent);
        }
        let shape = match self.kind {
            ParentReferenceKindV1::Product => ChildShapeV1::Product {
                rows: self.parent_a.ordinary_count,
                columns: self.parent_b.ordinary_count,
            },
            ParentReferenceKindV1::Conditional => {
                if self.condition == self.parent_a.ordinary_count {
                    return Err(ParentRefusalV1::ConditionOnFailure);
                }
                if self.parent_a.ordinary_count < self.condition {
                    return Err(ParentRefusalV1::ConditionOutOfRange);
                }
                ChildShapeV1::Conditional {
                    rows: self.parent_a.ordinary_count,
                    condition: self.condition,
                    branch: self.parent_b.ordinary_count,
                }
            }
        };
        if CHILD_MAXIMUM_OUTCOME_COUNT_V1 < shape.width()? {
            return Err(ParentRefusalV1::WidthOverflow);
        }
        Ok(shape)
    }
}

/// The child's shape, read off an admitted reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildShapeV1 {
    /// `A × B`, row-major with `A` major: cell `(a, b)` at `a · columns + b`.
    Product {
        /// `R_A`.
        rows: u32,
        /// `R_B`.
        columns: u32,
    },
    /// `B | A = condition`: cells `0 … branch − 1` are the branch, cell
    /// `branch` is off-condition, `branch + 1` is the failure coordinate.
    Conditional {
        /// `R_A`.
        rows: u32,
        /// The ordinary outcome of `A` the branch is conditioned on.
        condition: u32,
        /// `R_B`.
        branch: u32,
    },
}

impl ChildShapeV1 {
    /// Ordinary cells: `R_A · R_B` or `R_B + 1`.
    pub fn ordinary_count(self) -> core::result::Result<u32, ParentRefusalV1> {
        match self {
            Self::Product { rows, columns } => rows
                .checked_mul(columns)
                .ok_or(ParentRefusalV1::WidthOverflow),
            Self::Conditional { branch, .. } => {
                branch.checked_add(1).ok_or(ParentRefusalV1::WidthOverflow)
            }
        }
    }

    /// Ordinary cells plus the child's own failure coordinate.
    pub fn width(self) -> core::result::Result<u32, ParentRefusalV1> {
        self.ordinary_count()?
            .checked_add(1)
            .ok_or(ParentRefusalV1::WidthOverflow)
    }

    /// The child's failure coordinate: its ordinary count.
    pub fn failure_selector(self) -> core::result::Result<u32, ParentRefusalV1> {
        self.ordinary_count()
    }

    /// The off-condition cell of a conditional child: `R_B`.
    pub const fn off_condition_cell(self) -> Option<u32> {
        match self {
            Self::Product { .. } => None,
            Self::Conditional { branch, .. } => Some(branch),
        }
    }

    /// The consecutive-integer cuts of the child's `ResultDomainV2`:
    /// `[1, …, n − 1]` over denominator one, so the joint index `v ∈ [0, n)`
    /// selects region `v` (`childCuts` in the Lean).
    pub fn cut_count(self) -> core::result::Result<u32, ParentRefusalV1> {
        self.ordinary_count()?
            .checked_sub(1)
            .ok_or(ParentRefusalV1::EmptyParent)
    }

    /// The `k`-th cut, `1 ≤ k + 1 ≤ n − 1`.
    pub fn cut(self, index: u32) -> core::result::Result<i128, ParentRefusalV1> {
        if index >= self.cut_count()? {
            return Err(ParentRefusalV1::WidthOverflow);
        }
        Ok(i128::from(index) + 1)
    }

    /// The refunding categorical basis scale the child is founded at:
    /// `payoutScale = ordinaryCount` (`categoricalRefundsOnFailure`).
    pub fn payout_scale(self) -> core::result::Result<u64, ParentRefusalV1> {
        Ok(u64::from(self.ordinary_count()?))
    }

    /// The product selector: `cell(a, b)` when both parents are ordinary, the
    /// child's failure coordinate when either is not (`productSelector`).
    pub fn product_selector(
        self,
        a: ParentTerminalV1,
        b: ParentTerminalV1,
    ) -> core::result::Result<u32, ParentRefusalV1> {
        let Self::Product { rows, columns } = self else {
            return Err(ParentRefusalV1::WidthOverflow);
        };
        if a.selector < rows && b.selector < columns {
            a.selector
                .checked_mul(columns)
                .and_then(|row| row.checked_add(b.selector))
                .ok_or(ParentRefusalV1::WidthOverflow)
        } else {
            self.failure_selector()
        }
    }

    /// The conditional selector (`conditionalSelector`): `A` failed ⇒ failure;
    /// `A ≠ condition` ⇒ off-condition WITHOUT reading `B`; `A = condition` ⇒
    /// `B`'s selector if ordinary, failure if `B` failed, `ParentNotTerminal`
    /// if `B` is not yet terminal.
    pub fn conditional_selector(
        self,
        a: ParentTerminalV1,
        b: Option<ParentTerminalV1>,
    ) -> core::result::Result<u32, ParentRefusalV1> {
        let Self::Conditional {
            rows,
            condition,
            branch,
        } = self
        else {
            return Err(ParentRefusalV1::WidthOverflow);
        };
        if rows <= a.selector {
            return self.failure_selector();
        }
        if a.selector != condition {
            return Ok(branch);
        }
        match b {
            None => Err(ParentRefusalV1::ParentNotTerminal),
            Some(b) if b.selector < branch => Ok(b.selector),
            Some(_) => self.failure_selector(),
        }
    }

    /// Whether settling needs parent `B`'s certificate at all: always for a
    /// product; for a conditional only on the condition branch.
    pub const fn needs_parent_b(self, a: ParentTerminalV1) -> bool {
        match self {
            Self::Product { .. } => true,
            Self::Conditional {
                rows, condition, ..
            } => a.selector < rows && a.selector == condition,
        }
    }

    /// The row of a product cell, for readers: `i / R_B`.
    pub const fn row(self, cell: u32) -> Option<u32> {
        match self {
            Self::Product { columns, .. } if columns != 0 => Some(cell / columns),
            _ => None,
        }
    }

    /// The column of a product cell, for readers: `i mod R_B`.
    pub const fn column(self, cell: u32) -> Option<u32> {
        match self {
            Self::Product { columns, .. } if columns != 0 => Some(cell % columns),
            _ => None,
        }
    }
}

/// Settle a child from two optional parent certificates: the Lean
/// `settleProduct` / `settleConditional`, one function over the kind.
pub fn settle_child_v1(
    reference: ParentReferenceV1,
    certificate_a: Option<ParentCertificateV1>,
    certificate_b: Option<ParentCertificateV1>,
) -> core::result::Result<u32, ParentRefusalV1> {
    let shape = reference.admit_founding()?;
    let a = admit_parent_certificate_v1(
        reference.parent_a,
        certificate_a.ok_or(ParentRefusalV1::ParentNotTerminal)?,
    )?;
    match shape {
        ChildShapeV1::Product { .. } => {
            let b = admit_parent_certificate_v1(
                reference.parent_b,
                certificate_b.ok_or(ParentRefusalV1::ParentNotTerminal)?,
            )?;
            shape.product_selector(a, b)
        }
        ChildShapeV1::Conditional { .. } => {
            // Off the condition `B` is not read, terminal or not
            // (`off_condition_ignores_B`); on it a live `B` is
            // `parentNotTerminal` and a terminal one is admitted.
            let b = match certificate_b {
                Some(certificate) if certificate.terminal => Some(admit_parent_certificate_v1(
                    reference.parent_b,
                    certificate,
                )?),
                _ => None,
            };
            shape.conditional_selector(a, b)
        }
    }
}

/// The derived provider's one instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedSettleRequestV1 {
    /// The child's immutable generation.
    pub generation: u64,
    /// The certificate sequence being written; `1` for the primary.
    pub terminal_sequence: u64,
    /// The `ParentReferenceV1` record's content identity.
    pub parent_reference: [u8; 32],
    /// The child's `SourceMaterialV3` content identity.
    pub source_material: [u8; 32],
    /// The child's `SourceSpecV1` content identity.
    pub source_spec: [u8; 32],
}

impl DerivedSettleRequestV1 {
    /// Hostile-decode one exactly 128-byte request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DERIVED_SETTLE_REQUEST_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, generated::DERIVED_SETTLE_MAGIC_OFFSET_V1)? != DERIVED_SETTLE_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, generated::DERIVED_SETTLE_VERSION_OFFSET_V1)?
            != DERIVED_SETTLE_VERSION_V1
            || byte(bytes, generated::DERIVED_SETTLE_ACTION_OFFSET_V1)? != DERIVED_SETTLE_ACTION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, generated::DERIVED_SETTLE_RESERVED_OFFSET_V1, 5)?;
        let value = Self {
            generation: read_u64(bytes, generated::DERIVED_SETTLE_GENERATION_OFFSET_V1)?,
            terminal_sequence: read_u64(
                bytes,
                generated::DERIVED_SETTLE_TERMINAL_SEQUENCE_OFFSET_V1,
            )?,
            parent_reference: array(bytes, generated::DERIVED_SETTLE_PARENT_REFERENCE_OFFSET_V1)?,
            source_material: array(bytes, generated::DERIVED_SETTLE_SOURCE_MATERIAL_OFFSET_V1)?,
            source_spec: array(bytes, generated::DERIVED_SETTLE_SOURCE_SPEC_OFFSET_V1)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; DERIVED_SETTLE_REQUEST_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; DERIVED_SETTLE_REQUEST_BYTES_V1];
        put(
            &mut output,
            generated::DERIVED_SETTLE_MAGIC_OFFSET_V1,
            &DERIVED_SETTLE_MAGIC_V1,
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_VERSION_OFFSET_V1,
            &DERIVED_SETTLE_VERSION_V1.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_ACTION_OFFSET_V1,
            &[DERIVED_SETTLE_ACTION_V1],
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_GENERATION_OFFSET_V1,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_TERMINAL_SEQUENCE_OFFSET_V1,
            &self.terminal_sequence.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_PARENT_REFERENCE_OFFSET_V1,
            &self.parent_reference,
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_SOURCE_MATERIAL_OFFSET_V1,
            &self.source_material,
        );
        put(
            &mut output,
            generated::DERIVED_SETTLE_SOURCE_SPEC_OFFSET_V1,
            &self.source_spec,
        );
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0 || self.terminal_sequence == 0 {
            return Err(Error::ZeroSequence);
        }
        if is_zero(&self.parent_reference)
            || is_zero(&self.source_material)
            || is_zero(&self.source_spec)
        {
            return Err(Error::ZeroContentId);
        }
        Ok(())
    }

    /// The reference identity as a content id.
    pub fn parent_reference_id(self) -> Result<ContentId> {
        ContentId::new(self.parent_reference)
    }
}

/// Whether bytes select the derived settle instruction.
pub fn is_derived_settle_v1(bytes: &[u8]) -> bool {
    bytes.len() == DERIVED_SETTLE_REQUEST_BYTES_V1
        && bytes.get(..8) == Some(DERIVED_SETTLE_MAGIC_V1.as_slice())
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

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let span = bytes
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?;
    if span.iter().any(|value| *value != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|value| *value == 0)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(value.iter().copied()) {
        *destination = source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_parent_reference_v1::{
        DERIVED_SETTLE_REFUSAL_CORPUS_V1, DERIVED_SETTLE_REFUSAL_CORPUS_V1_COUNT,
        DERIVED_SETTLE_REQUEST_EXAMPLE_V1, PARENT_REFERENCE_CONDITIONAL_EXAMPLE_V1,
        PARENT_REFERENCE_PRODUCT_EXAMPLE_V1, PARENT_REFERENCE_REFUSAL_CORPUS_V1,
        PARENT_REFERENCE_REFUSAL_CORPUS_V1_COUNT,
    };

    fn id(tag: u8) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        bytes
    }

    /// The Lean `Examples.parentA` / `parentB`: two width-3 parents.
    fn parent_a() -> ParentRefV1 {
        ParentRefV1 {
            market: id(0xA),
            generation: 1,
            product_record_digest: id(0xA1),
            ordinary_count: 2,
        }
    }

    fn parent_b() -> ParentRefV1 {
        ParentRefV1 {
            market: id(0xB),
            generation: 1,
            product_record_digest: id(0xB1),
            ordinary_count: 2,
        }
    }

    fn product() -> ParentReferenceV1 {
        ParentReferenceV1 {
            kind: ParentReferenceKindV1::Product,
            parent_a: parent_a(),
            parent_b: parent_b(),
            condition: 0,
            settle_by: 1_800_000_000,
        }
    }

    fn conditional() -> ParentReferenceV1 {
        ParentReferenceV1 {
            kind: ParentReferenceKindV1::Conditional,
            ..product()
        }
    }

    fn certificate(reference: ParentRefV1, selector: u32) -> ParentCertificateV1 {
        ParentCertificateV1 {
            market: reference.market,
            generation: reference.generation,
            product_record_digest: reference.product_record_digest,
            ordinary_count: reference.ordinary_count,
            selector,
            terminal: true,
        }
    }

    #[test]
    fn the_examples_are_the_emitted_bytes() {
        assert_eq!(
            product().to_bytes().expect("product"),
            PARENT_REFERENCE_PRODUCT_EXAMPLE_V1
        );
        assert_eq!(
            conditional().to_bytes().expect("conditional"),
            PARENT_REFERENCE_CONDITIONAL_EXAMPLE_V1
        );
        assert_eq!(
            ParentReferenceV1::decode(&PARENT_REFERENCE_PRODUCT_EXAMPLE_V1).expect("decode"),
            product()
        );
        assert_eq!(
            ParentReferenceV1::decode(&PARENT_REFERENCE_CONDITIONAL_EXAMPLE_V1).expect("decode"),
            conditional()
        );
    }

    #[test]
    fn the_emitted_corpus_refuses_row_by_row() {
        assert_eq!(
            PARENT_REFERENCE_REFUSAL_CORPUS_V1.len(),
            PARENT_REFERENCE_REFUSAL_CORPUS_V1_COUNT
        );
        for (index, candidate) in PARENT_REFERENCE_REFUSAL_CORPUS_V1.iter().enumerate() {
            // The row is a hostile before it is a refusal. A mutation that
            // lands on a byte already holding the mutated value re-emits the
            // valid record, and a corpus that carries one is refused by
            // nothing while looking exactly like a corpus that is.
            assert_ne!(
                candidate.as_slice(),
                PARENT_REFERENCE_PRODUCT_EXAMPLE_V1.as_slice(),
                "corpus row {index} is the valid record"
            );
            assert!(
                ParentReferenceV1::decode(candidate).is_err(),
                "corpus row {index} decoded"
            );
        }
        assert_eq!(
            DERIVED_SETTLE_REFUSAL_CORPUS_V1.len(),
            DERIVED_SETTLE_REFUSAL_CORPUS_V1_COUNT
        );
        for (index, candidate) in DERIVED_SETTLE_REFUSAL_CORPUS_V1.iter().enumerate() {
            assert_ne!(
                candidate.as_slice(),
                DERIVED_SETTLE_REQUEST_EXAMPLE_V1.as_slice(),
                "request corpus row {index} is the valid request"
            );
            assert!(
                DerivedSettleRequestV1::decode(candidate).is_err(),
                "request corpus row {index} decoded"
            );
        }
    }

    #[test]
    fn the_request_example_round_trips() {
        let request =
            DerivedSettleRequestV1::decode(&DERIVED_SETTLE_REQUEST_EXAMPLE_V1).expect("example");
        assert_eq!(request.generation, 2);
        assert_eq!(request.terminal_sequence, 1);
        assert_eq!(
            request.to_bytes().expect("encode"),
            DERIVED_SETTLE_REQUEST_EXAMPLE_V1
        );
        assert!(is_derived_settle_v1(&DERIVED_SETTLE_REQUEST_EXAMPLE_V1));
        assert!(!is_derived_settle_v1(
            &DERIVED_SETTLE_REQUEST_EXAMPLE_V1[..127]
        ));
    }

    /// `product.cells = 4`, `width = 5`, `failureSelector = 4`.
    #[test]
    fn the_product_example_shape() {
        let shape = product().admit_founding().expect("found");
        assert_eq!(shape.ordinary_count(), Ok(4));
        assert_eq!(shape.width(), Ok(5));
        assert_eq!(shape.failure_selector(), Ok(4));
        assert_eq!(shape.cut_count(), Ok(3));
        assert_eq!(shape.cut(0), Ok(1));
        assert_eq!(shape.cut(2), Ok(3));
        assert!(shape.cut(3).is_err());
        assert_eq!(shape.payout_scale(), Ok(4));
    }

    /// `(1, 0)` is cell 2; `A` on its failure coordinate fails the child.
    #[test]
    fn the_product_settles_as_the_lean_examples() {
        let reference = product();
        assert_eq!(
            settle_child_v1(
                reference,
                Some(certificate(parent_a(), 1)),
                Some(certificate(parent_b(), 0))
            ),
            Ok(2)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(certificate(parent_a(), 2)),
                Some(certificate(parent_b(), 0))
            ),
            Ok(4)
        );
        let shape = reference.admit_founding().expect("found");
        assert_eq!(shape.row(2), Some(1));
        assert_eq!(shape.column(2), Some(0));
    }

    /// `conditional.width = 4`, off-condition cell 2, failure 3; on the
    /// branch the selector is the product cell less the row offset; off it
    /// the child settles with `B` live; `B`'s outage is the child's only on
    /// the branch.
    #[test]
    fn the_conditional_settles_as_the_lean_examples() {
        let reference = conditional();
        let shape = reference.admit_founding().expect("found");
        assert_eq!(shape.width(), Ok(4));
        assert_eq!(shape.off_condition_cell(), Some(2));
        assert_eq!(shape.failure_selector(), Ok(3));
        assert_eq!(
            settle_child_v1(
                reference,
                Some(certificate(parent_a(), 0)),
                Some(certificate(parent_b(), 1))
            ),
            Ok(1)
        );
        assert_eq!(
            settle_child_v1(reference, Some(certificate(parent_a(), 1)), None),
            Ok(2)
        );
        assert_eq!(
            settle_child_v1(reference, Some(certificate(parent_a(), 0)), None),
            Err(ParentRefusalV1::ParentNotTerminal)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(certificate(parent_a(), 1)),
                Some(certificate(parent_b(), 2))
            ),
            Ok(2)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(certificate(parent_a(), 0)),
                Some(certificate(parent_b(), 2))
            ),
            Ok(3)
        );
    }

    #[test]
    fn the_hostiles_are_refused_by_name() {
        let reference = product();
        let a = certificate(parent_a(), 0);
        let b = certificate(parent_b(), 0);
        assert_eq!(
            settle_child_v1(reference, None, Some(b)),
            Err(ParentRefusalV1::ParentNotTerminal)
        );
        assert_eq!(
            settle_child_v1(reference, Some(a), None),
            Err(ParentRefusalV1::ParentNotTerminal)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(ParentCertificateV1 {
                    terminal: false,
                    ..a
                }),
                Some(b)
            ),
            Err(ParentRefusalV1::ParentNotTerminal)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(ParentCertificateV1 { generation: 2, ..a }),
                Some(b)
            ),
            Err(ParentRefusalV1::ParentGenerationMismatch)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(ParentCertificateV1 {
                    market: id(0xC),
                    ..a
                }),
                Some(b)
            ),
            Err(ParentRefusalV1::WrongParent)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(ParentCertificateV1 {
                    product_record_digest: id(0xA2),
                    ..a
                }),
                Some(b)
            ),
            Err(ParentRefusalV1::ParentRecordMismatch)
        );
        assert_eq!(
            settle_child_v1(
                reference,
                Some(ParentCertificateV1 {
                    ordinary_count: 3,
                    ..a
                }),
                Some(b)
            ),
            Err(ParentRefusalV1::ParentWidthMismatch)
        );
        assert_eq!(
            settle_child_v1(reference, Some(certificate(parent_a(), 3)), Some(b)),
            Err(ParentRefusalV1::SelectorOutOfRange)
        );
        assert_eq!(
            ParentReferenceV1 {
                condition: 2,
                ..conditional()
            }
            .admit_founding(),
            Err(ParentRefusalV1::ConditionOnFailure)
        );
        assert_eq!(
            ParentReferenceV1 {
                condition: 3,
                ..conditional()
            }
            .admit_founding(),
            Err(ParentRefusalV1::ConditionOutOfRange)
        );
        assert_eq!(
            ParentReferenceV1 {
                parent_b: parent_a(),
                ..product()
            }
            .admit_founding(),
            Err(ParentRefusalV1::SameParent)
        );
    }

    /// `7 × 8` fits at width 57; `8 × 8` does not at 65.
    #[test]
    fn the_bank_cap_matches_the_lean() {
        let wide = |a: u32, b: u32| ParentReferenceV1 {
            parent_a: ParentRefV1 {
                ordinary_count: a,
                ..parent_a()
            },
            parent_b: ParentRefV1 {
                ordinary_count: b,
                ..parent_b()
            },
            ..product()
        };
        let seven_by_eight = wide(7, 8).admit_founding().expect("fits");
        assert_eq!(seven_by_eight.width(), Ok(57));
        assert_eq!(
            wide(8, 8).admit_founding(),
            Err(ParentRefusalV1::WidthOverflow)
        );
        assert_eq!(CHILD_MAXIMUM_OUTCOME_COUNT_V1, 60);
    }

    #[test]
    fn a_product_record_carrying_a_condition_is_not_canonical() {
        let mut bytes = PARENT_REFERENCE_PRODUCT_EXAMPLE_V1;
        bytes[generated::PARENT_REFERENCE_CONDITION_OFFSET_V1] = 1;
        assert_eq!(
            ParentReferenceV1::decode(&bytes),
            Err(Error::NonCanonicalReservedBytes)
        );
    }

    #[test]
    fn the_settle_frame_roles_are_the_emitted_table() {
        assert_eq!(
            DERIVED_SETTLE_ACCOUNT_ROLES_V1.len(),
            DERIVED_SETTLE_ACCOUNT_COUNT_V1
        );
        assert_eq!(
            DERIVED_SETTLE_ACCOUNT_COUNT_V1 - DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1,
            DERIVED_SETTLE_PARENT_SLOT_COUNT_V1
        );
        let signers = DERIVED_SETTLE_ACCOUNT_ROLES_V1
            .iter()
            .filter(|(_, signer)| *signer)
            .count();
        assert_eq!(signers, 1);
        for (index, (writable, _)) in DERIVED_SETTLE_ACCOUNT_ROLES_V1.iter().enumerate() {
            if index >= DERIVED_SETTLE_PARENT_A_MARKET_INDEX_V1 {
                assert!(!writable, "parent slot {index} is writable");
            }
        }
    }
}
