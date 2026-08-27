//! Canonical root translation and complete admitted-bundle joins.

use crate::CompositionDescriptorV3;
use crate::abi::{
    COMPOSITION_SCHEMA_VERSION_V3, COMPOSITION_TRANSLATION_HEADER_BYTES_V3,
    COMPOSITION_TRANSLATION_MAGIC_V3, Error, RecordAdmissionV3, Result, array_at, gcd_u64,
    nonzero_array, put, require_zero, slice, u16_at, u32_at, u64_at, validate_record_admission,
};
use crate::graph::{COMPOSITION_TERM_BYTES_V3, CompositionGraphV3, SparseTermV3, TermLayoutV3};

/// Translation-header byte-layout authority.
pub struct TranslationLayoutV3;

impl TranslationLayoutV3 {
    /// Magic offset.
    pub const MAGIC: usize = 0;
    /// Schema-version offset.
    pub const VERSION: usize = 8;
    /// Reserved header offset.
    pub const RESERVED_HEADER: usize = 10;
    /// Stable graph identity offset.
    pub const GRAPH_ID: usize = 16;
    /// Sole graph-root identity offset.
    pub const ROOT_ID: usize = 48;
    /// Exhaustive native width offset.
    pub const OUTCOME_COUNT: usize = 80;
    /// Sparse root-term count offset.
    pub const TERM_COUNT: usize = 84;
    /// Canonical common denominator offset.
    pub const DENOMINATOR: usize = 88;
    /// Reserved tail offset.
    pub const RESERVED_TAIL: usize = 96;
}

/// Borrowed canonical translation supplied to the atomic encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalTranslationInputV3<'a> {
    /// Stable graph identity.
    pub graph_id: [u8; 32],
    /// Sole graph-root identity.
    pub root_id: [u8; 32],
    /// Exhaustive native width.
    pub outcome_count: u32,
    /// Canonical root common denominator.
    pub denominator: u64,
    /// Strictly ordered positive sparse payoff terms.
    pub terms: &'a [SparseTermV3],
}

/// Hostile-decoded canonical root-to-native translation witness.
#[derive(Clone, Copy)]
pub struct CanonicalTranslationV3<'a> {
    bytes: &'a [u8],
    graph_id: [u8; 32],
    root_id: [u8; 32],
    outcome_count: u32,
    term_count: u32,
    denominator: u64,
}

impl<'a> CanonicalTranslationV3<'a> {
    fn decode_structural(input: &'a [u8]) -> Result<Self> {
        if input.len() < COMPOSITION_TRANSLATION_HEADER_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, TranslationLayoutV3::MAGIC)? != COMPOSITION_TRANSLATION_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, TranslationLayoutV3::VERSION)? != COMPOSITION_SCHEMA_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, TranslationLayoutV3::RESERVED_HEADER, 6)?;
        require_zero(input, TranslationLayoutV3::RESERVED_TAIL, 32)?;
        let translation = Self {
            bytes: input,
            graph_id: nonzero_array(input, TranslationLayoutV3::GRAPH_ID)?,
            root_id: nonzero_array(input, TranslationLayoutV3::ROOT_ID)?,
            outcome_count: u32_at(input, TranslationLayoutV3::OUTCOME_COUNT)?,
            term_count: u32_at(input, TranslationLayoutV3::TERM_COUNT)?,
            denominator: u64_at(input, TranslationLayoutV3::DENOMINATOR)?,
        };
        translation.validate_structural()?;
        Ok(translation)
    }

    /// Exact admitted translation bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Stable graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }

    /// Sole root identity.
    pub const fn root_id(self) -> [u8; 32] {
        self.root_id
    }

    /// Exhaustive native width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Sparse root term count.
    pub const fn term_count(self) -> u32 {
        self.term_count
    }

    /// Canonical common denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Read one sparse term.
    pub fn term(self, index: u32) -> Result<SparseTermV3> {
        if index >= self.term_count {
            return Err(Error::InvalidLength);
        }
        let offset = COMPOSITION_TRANSLATION_HEADER_BYTES_V3
            .checked_add(
                usize::try_from(index)
                    .map_err(|_| Error::InvalidLength)?
                    .checked_mul(COMPOSITION_TERM_BYTES_V3)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let bytes = slice(self.bytes, offset, COMPOSITION_TERM_BYTES_V3)?;
        require_zero(bytes, TermLayoutV3::RESERVED, 4)?;
        Ok(SparseTermV3 {
            outcome: u32_at(bytes, TermLayoutV3::OUTCOME)?,
            numerator: u64_at(bytes, TermLayoutV3::NUMERATOR)?,
        })
    }

    /// Materialize an exact root quantity into exhaustive native quantities atomically.
    pub fn materialize_exact(
        self,
        root_quantity: u64,
        scratch: &mut [u64],
        output: &mut [u64],
    ) -> Result<()> {
        let width = usize::try_from(self.outcome_count).map_err(|_| Error::InvalidOutcome)?;
        if scratch.len() != width || output.len() != width {
            return Err(Error::InvalidLength);
        }
        scratch.fill(0);
        let mut index = 0_u32;
        while index < self.term_count {
            let term = self.term(index)?;
            let product = u128::from(root_quantity)
                .checked_mul(u128::from(term.numerator))
                .ok_or(Error::ArithmeticOverflow)?;
            let denominator = u128::from(self.denominator);
            if product % denominator != 0 {
                return Err(Error::NonIntegralTranslation);
            }
            let exact = product / denominator;
            let narrowed = u64::try_from(exact).map_err(|_| Error::ArithmeticOverflow)?;
            let coordinate = usize::try_from(term.outcome).map_err(|_| Error::InvalidOutcome)?;
            *scratch.get_mut(coordinate).ok_or(Error::InvalidOutcome)? = narrowed;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        output.copy_from_slice(scratch);
        Ok(())
    }

    /// Prove supplied native quantities equal the exact root translation without rounding.
    pub fn verify_conservation(self, root_quantity: u64, native: &[u64]) -> Result<()> {
        let width = usize::try_from(self.outcome_count).map_err(|_| Error::InvalidOutcome)?;
        if native.len() != width {
            return Err(Error::InvalidLength);
        }
        let mut term_index = 0_u32;
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let term = if term_index < self.term_count {
                Some(self.term(term_index)?)
            } else {
                None
            };
            let expected_numerator = if term.is_some_and(|value| value.outcome == outcome) {
                term_index = term_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                u128::from(term.ok_or(Error::TranslationMismatch)?.numerator)
            } else {
                0
            };
            let coordinate = usize::try_from(outcome).map_err(|_| Error::InvalidOutcome)?;
            let observed = u128::from(*native.get(coordinate).ok_or(Error::InvalidOutcome)?)
                .checked_mul(u128::from(self.denominator))
                .ok_or(Error::ArithmeticOverflow)?;
            let expected = u128::from(root_quantity)
                .checked_mul(expected_numerator)
                .ok_or(Error::ArithmeticOverflow)?;
            if observed != expected {
                return Err(Error::ConservationMismatch);
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn validate_structural(self) -> Result<()> {
        if self.outcome_count < crate::MIN_COMPOSITION_OUTCOMES_V3
            || self.outcome_count > crate::MAX_COMPOSITION_OUTCOMES_V3
        {
            return Err(Error::InvalidOutcome);
        }
        if self.term_count == 0 || self.denominator == 0 {
            return Err(Error::NonCanonicalPayoff);
        }
        if self.bytes.len() != composition_translation_bytes_v3(self.term_count)? {
            return Err(Error::InvalidLength);
        }
        let mut prior = None;
        let mut normalization = self.denominator;
        let mut index = 0_u32;
        while index < self.term_count {
            let term = self.term(index)?;
            if term.outcome >= self.outcome_count
                || term.numerator == 0
                || prior.is_some_and(|value| term.outcome <= value)
            {
                return Err(Error::NonCanonicalPayoff);
            }
            normalization = gcd_u64(normalization, term.numerator);
            prior = Some(term.outcome);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if normalization != 1 {
            return Err(Error::NonCanonicalPayoff);
        }
        Ok(())
    }
}

/// One completely joined descriptor, graph, and byte-identical root translation.
#[derive(Clone, Copy)]
pub struct CompositionBundleV3<'a> {
    descriptor: CompositionDescriptorV3,
    graph: CompositionGraphV3<'a>,
    translation: CanonicalTranslationV3<'a>,
}

impl<'a> CompositionBundleV3<'a> {
    /// Admitted immutable descriptor.
    pub const fn descriptor(self) -> CompositionDescriptorV3 {
        self.descriptor
    }

    /// Independently validated acyclic graph.
    pub const fn graph(self) -> CompositionGraphV3<'a> {
        self.graph
    }

    /// Byte-identical canonical root translation.
    pub const fn translation(self) -> CanonicalTranslationV3<'a> {
        self.translation
    }
}

/// Decode one fully authenticated composition bundle and all cross-record joins.
pub fn decode_composition_bundle_v3<'a>(
    descriptor_bytes: &[u8],
    descriptor_admission: RecordAdmissionV3,
    graph_bytes: &'a [u8],
    graph_admission: RecordAdmissionV3,
    translation_bytes: &'a [u8],
    translation_admission: RecordAdmissionV3,
) -> Result<CompositionBundleV3<'a>> {
    let descriptor = CompositionDescriptorV3::decode(descriptor_bytes, descriptor_admission)?;
    let graph = CompositionGraphV3::decode(graph_bytes, descriptor, graph_admission)?;
    validate_record_admission(
        translation_admission,
        descriptor.translation_id(),
        descriptor.translation_digest(),
    )?;
    let translation = CanonicalTranslationV3::decode_structural(translation_bytes)?;
    if translation.graph_id != descriptor.graph_id()
        || translation.root_id != descriptor.root_id()
        || translation.outcome_count != descriptor.outcome_count()
        || translation.term_count != graph.root_term_count()?
        || translation.denominator != descriptor.root_denominator()
        || translation.denominator != graph.root_denominator()?
        || slice(
            translation.bytes,
            COMPOSITION_TRANSLATION_HEADER_BYTES_V3,
            graph.root_term_bytes()?.len(),
        )? != graph.root_term_bytes()?
    {
        return Err(Error::TranslationMismatch);
    }
    Ok(CompositionBundleV3 {
        descriptor,
        graph,
        translation,
    })
}

/// Return the exact translation width for a validated sparse-term count.
pub fn composition_translation_bytes_v3(term_count: u32) -> Result<usize> {
    if term_count == 0 || term_count > crate::MAX_COMPOSITION_TERMS_V3 {
        return Err(Error::CapacityExceeded);
    }
    COMPOSITION_TRANSLATION_HEADER_BYTES_V3
        .checked_add(
            usize::try_from(term_count)
                .map_err(|_| Error::InvalidLength)?
                .checked_mul(COMPOSITION_TERM_BYTES_V3)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)
}

/// Encode one canonical root translation atomically.
pub fn encode_canonical_translation_v3_atomic(
    input: CanonicalTranslationInputV3<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let term_count = u32::try_from(input.terms.len()).map_err(|_| Error::CapacityExceeded)?;
    let length = composition_translation_bytes_v3(term_count)?;
    if scratch.len() != length || output.len() != length {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    put(
        scratch,
        TranslationLayoutV3::MAGIC,
        &COMPOSITION_TRANSLATION_MAGIC_V3,
    )?;
    put(
        scratch,
        TranslationLayoutV3::VERSION,
        &COMPOSITION_SCHEMA_VERSION_V3.to_le_bytes(),
    )?;
    put(scratch, TranslationLayoutV3::GRAPH_ID, &input.graph_id)?;
    put(scratch, TranslationLayoutV3::ROOT_ID, &input.root_id)?;
    put(
        scratch,
        TranslationLayoutV3::OUTCOME_COUNT,
        &input.outcome_count.to_le_bytes(),
    )?;
    put(
        scratch,
        TranslationLayoutV3::TERM_COUNT,
        &term_count.to_le_bytes(),
    )?;
    put(
        scratch,
        TranslationLayoutV3::DENOMINATOR,
        &input.denominator.to_le_bytes(),
    )?;
    for (index, term) in input.terms.iter().enumerate() {
        let offset = COMPOSITION_TRANSLATION_HEADER_BYTES_V3
            .checked_add(
                index
                    .checked_mul(COMPOSITION_TERM_BYTES_V3)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        put(
            scratch,
            offset + TermLayoutV3::OUTCOME,
            &term.outcome.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + TermLayoutV3::NUMERATOR,
            &term.numerator.to_le_bytes(),
        )?;
    }
    let decoded = CanonicalTranslationV3::decode_structural(scratch)?;
    if decoded.graph_id != input.graph_id || decoded.root_id != input.root_id {
        return Err(Error::TranslationMismatch);
    }
    output.copy_from_slice(scratch);
    Ok(())
}
