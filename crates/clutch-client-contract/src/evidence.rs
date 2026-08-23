//! Evidence provenance labels and conservative temporal promotion.

/// Exact provenance of one client-visible observation.
///
/// Provenance is not claim strength. For example, an observation can be
/// transaction-derived while still coming from an unpromoted local validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceProvenance {
    /// Derived from authenticated current chain account state.
    ChainDerived,
    /// Derived from an authenticated retained chain-history source.
    ChainHistoryDerived,
    /// Derived from a retained transaction and its execution result.
    TransactionDerived,
    /// Asserted by a producer, without independent chain derivation.
    ProducerAttested,
    /// Produced by a model, fixture, or simulation only.
    ModelOnly,
    /// No evidence is available for the projected field.
    Unavailable,
}

impl EvidenceProvenance {
    /// Exact stable label used by clients and serialized projections.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChainDerived => "chain-derived",
            Self::ChainHistoryDerived => "chain-history-derived",
            Self::TransactionDerived => "transaction-derived",
            Self::ProducerAttested => "producer-attested",
            Self::ModelOnly => "model-only",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Time range actually retained by an observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceScope {
    /// One current-state snapshot, with no retained event history.
    CurrentSnapshot,
    /// Retained history covering the event being described.
    RetainedHistory,
}

/// One declared provenance label paired with its actual temporal scope.
///
/// Construction does not authenticate either declaration. The client adapter
/// that observed the source must validate and bind it independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceDescriptor {
    provenance: EvidenceProvenance,
    scope: EvidenceScope,
}

impl EvidenceDescriptor {
    /// Construct a descriptor without promoting its claims.
    #[must_use]
    pub const fn new(provenance: EvidenceProvenance, scope: EvidenceScope) -> Self {
        Self { provenance, scope }
    }

    /// Exact provenance.
    #[must_use]
    pub const fn provenance(self) -> EvidenceProvenance {
        self.provenance
    }

    /// Exact temporal scope.
    #[must_use]
    pub const fn scope(self) -> EvidenceScope {
        self.scope
    }

    /// Attempt to retain this descriptor as a historical source.
    ///
    /// A current snapshot always refuses: absent accounts or terminal current
    /// state cannot reconstruct deleted receipts or prove the path taken.
    /// Model-only, unavailable, and producer-only declarations also cannot be
    /// promoted through this independent client contract. Success means only
    /// that the source can participate in a later subject-specific exhaustive
    /// join; it says nothing was completed by itself.
    ///
    /// # Errors
    ///
    /// Refuses a current snapshot or provenance that is unavailable,
    /// model-only, producer-attested, or merely current-chain-derived.
    pub const fn retained_historical_source(
        self,
    ) -> Result<RetainedHistoricalSource, EvidenceRefusal> {
        match self.scope {
            EvidenceScope::CurrentSnapshot => {
                return Err(EvidenceRefusal::FreshSnapshotHasNoHistory);
            }
            EvidenceScope::RetainedHistory => {}
        }
        match self.provenance {
            EvidenceProvenance::ChainHistoryDerived | EvidenceProvenance::TransactionDerived => {
                Ok(RetainedHistoricalSource { source: self })
            }
            EvidenceProvenance::ChainDerived
            | EvidenceProvenance::ProducerAttested
            | EvidenceProvenance::ModelOnly
            | EvidenceProvenance::Unavailable => Err(EvidenceRefusal::InsufficientProvenance),
        }
    }
}

/// A descriptor conservatively admitted as one retained historical source.
///
/// This is not a completion certificate. A subject-specific client must still
/// prove that its retained sources are canonical and exhaustive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedHistoricalSource {
    source: EvidenceDescriptor,
}

impl RetainedHistoricalSource {
    /// The exact retained-history descriptor that passed admission.
    #[must_use]
    pub const fn source(self) -> EvidenceDescriptor {
        self.source
    }
}

/// Refusal to promote an observation into an independent historical source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceRefusal {
    /// Current state does not retain the event history needed by the claim.
    FreshSnapshotHasNoHistory,
    /// The provenance is not independently sufficient for this promotion.
    InsufficientProvenance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_provenance_labels_are_frozen() {
        let cases = [
            (EvidenceProvenance::ChainDerived, "chain-derived"),
            (
                EvidenceProvenance::ChainHistoryDerived,
                "chain-history-derived",
            ),
            (
                EvidenceProvenance::TransactionDerived,
                "transaction-derived",
            ),
            (EvidenceProvenance::ProducerAttested, "producer-attested"),
            (EvidenceProvenance::ModelOnly, "model-only"),
            (EvidenceProvenance::Unavailable, "unavailable"),
        ];
        for (provenance, expected) in cases {
            assert_eq!(provenance.label(), expected);
        }
    }

    #[test]
    fn fresh_snapshots_never_become_retained_historical_sources() {
        for provenance in [
            EvidenceProvenance::ChainDerived,
            EvidenceProvenance::ChainHistoryDerived,
            EvidenceProvenance::TransactionDerived,
            EvidenceProvenance::ProducerAttested,
            EvidenceProvenance::ModelOnly,
            EvidenceProvenance::Unavailable,
        ] {
            assert_eq!(
                EvidenceDescriptor::new(provenance, EvidenceScope::CurrentSnapshot)
                    .retained_historical_source(),
                Err(EvidenceRefusal::FreshSnapshotHasNoHistory)
            );
        }
    }

    #[test]
    fn only_independent_retained_history_becomes_a_historical_source() {
        for provenance in [
            EvidenceProvenance::ChainHistoryDerived,
            EvidenceProvenance::TransactionDerived,
        ] {
            let descriptor = EvidenceDescriptor::new(provenance, EvidenceScope::RetainedHistory);
            assert_eq!(
                descriptor
                    .retained_historical_source()
                    .expect("retained independent history is admitted")
                    .source(),
                descriptor
            );
        }
        for provenance in [
            EvidenceProvenance::ChainDerived,
            EvidenceProvenance::ProducerAttested,
            EvidenceProvenance::ModelOnly,
            EvidenceProvenance::Unavailable,
        ] {
            assert_eq!(
                EvidenceDescriptor::new(provenance, EvidenceScope::RetainedHistory)
                    .retained_historical_source(),
                Err(EvidenceRefusal::InsufficientProvenance)
            );
        }
    }
}
